//! Delegation handlers.
//!
//!   POST   /dashboard/v1/delegations
//!   GET    /dashboard/v1/delegations
//!   GET    /dashboard/v1/delegations/:id_b64
//!   POST   /dashboard/v1/delegations/:id_b64/revoke

use std::sync::Arc;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use maat::scope::ScopeExpr;
use maat::{Delegation, ObjectId, PublicKey, SignatureAlgorithm};
use maat_config::{CreateDelegation, ConfigError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{bad_request, internal_error, not_found};
use crate::kms_signer::KmsSigner;
use crate::middleware::{require_admin, AuthContext};
use crate::state::DashboardState;

#[derive(Debug, Deserialize)]
pub struct CreateDelegationRequest {
    /// Tenant principal key (UUID from the principal_keys table) to sign
    /// the delegation with.
    pub principal_key_id: Uuid,

    /// Agent's public key, base64url-encoded (32 bytes for Ed25519).
    pub agent_pubkey_b64: String,

    /// Scope grants. e.g., ["finance:payment:execute"]
    pub scope_grants: Vec<String>,

    /// Unix-seconds. Optional; defaults to "now".
    pub not_before: Option<u64>,

    /// Unix-seconds. Required.
    pub not_after: u64,

    /// Slice 7: optional protocol-level constraints. The existing
    /// `Constraint` enum from the maat library. Signed into the
    /// delegation; enforced by the gateway's verify path.
    #[serde(default)]
    pub constraints: Vec<maat::Constraint>,

    /// Slice 7: optional gateway-side cumulative cap.
    /// NOT signed into the delegation — this is a layered cap stored
    /// alongside the delegation, enforced by the gateway's ledger.
    #[serde(default)]
    pub cumulative_cap: Option<crate::ledger::CumulativeCap>,
}

#[derive(Debug, Serialize)]
pub struct DelegationView {
    pub id_b64: String,
    pub delegation: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revocation_reason: Option<String>,
}

pub async fn create_delegation(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Json(req): Json<CreateDelegationRequest>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }

    // 1. Look up the principal key in this tenant. (Not someone else's tenant.)
    let pk = match state.principal_keys.get(auth.tenant_id, req.principal_key_id).await {
        Ok(p) => p,
        Err(ConfigError::NotFound) => return not_found("principal key not found"),
        Err(e) => return internal_error(&e.to_string()),
    };
    if pk.retired_at.is_some() {
        return bad_request("principal key is retired");
    }

    // 2. Decode the agent public key.
    let agent_key_bytes = match URL_SAFE_NO_PAD.decode(&req.agent_pubkey_b64) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => return bad_request("agent_pubkey_b64 must decode to 32 bytes"),
        Err(_) => return bad_request("agent_pubkey_b64 must be base64url-encoded"),
    };
    let agent_pubkey = PublicKey {
        algorithm: SignatureAlgorithm::Ed25519,
        key_data: agent_key_bytes,
    };

    // 3. Validate temporal bounds.
    let now = match maat::types::now() {
        Ok(n) => n,
        Err(e) => return internal_error(&format!("clock error: {}", e)),
    };
    let not_before = req.not_before.unwrap_or(now);
    if req.not_after <= not_before {
        return bad_request("not_after must be greater than not_before");
    }
    if req.not_after <= now {
        return bad_request("not_after must be in the future");
    }

    // 4. Validate scope grants are non-empty and well-formed.
    if req.scope_grants.is_empty() {
        return bad_request("scope_grants must not be empty");
    }
    for s in &req.scope_grants {
        if s.is_empty() {
            return bad_request("scope_grants must not contain empty strings");
        }
    }
    let scope = ScopeExpr::new(req.scope_grants.to_vec());

    // 5. Build the KMS signer using this tenant's principal key.
    let signer = match KmsSigner::new(state.kms.clone(), &pk.kms_key_id, &pk.public_key_b64) {
        Ok(s) => s,
        Err(e) => return internal_error(&format!("signer setup: {}", e)),
    };

    // 6. Build and sign the delegation via Maat's builder.
    let delegation = match Delegation::builder(agent_pubkey.clone(), scope.clone())
        .constraints(req.constraints.clone())
        .not_before(not_before)
        .not_after(req.not_after)
        .build(&signer)
    {
        Ok(d) => d,
        Err(e) => return internal_error(&format!("delegation build failed: {}", e)),
    };

    // 7. Persist.
    let delegation_json = match serde_json::to_value(&delegation) {
        Ok(v) => v,
        Err(e) => return internal_error(&format!("serialize failed: {}", e)),
    };
    let not_before_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(not_before as i64, 0)
        .unwrap_or_else(chrono::Utc::now);
    let not_after_dt = chrono::DateTime::<chrono::Utc>::from_timestamp(req.not_after as i64, 0)
        .unwrap_or_else(chrono::Utc::now);

    let record = match state
        .delegations
        .insert(CreateDelegation {
            id: &delegation.id.0,
            tenant_id: auth.tenant_id,
            principal_pubkey: &delegation.principal.key_data,
            agent_pubkey: &delegation.agent.key_data,
            parent_id: None,
            not_before: not_before_dt,
            not_after: not_after_dt,
            scope_grants: &req.scope_grants,
            delegation_json: &delegation_json,
        })
        .await
    {
        Ok(r) => r,
        Err(e) => return internal_error(&format!("persist failed: {}", e)),
    };

    // Slice 7: write the cumulative cap row if provided. Best-effort —
    // a failure here logs and surfaces a 500, since the delegation is
    // already persisted but its cap promise can't be honored.
    if let Some(cap) = &req.cumulative_cap {
        if let Err(e) =
            crate::ledger::set_cap(&state.pool, auth.tenant_id, &delegation.id.0, cap).await
        {
            tracing::error!(error = %e, "cumulative cap write failed");
            return internal_error(&format!("cumulative cap write failed: {}", e));
        }
    }

    let view = DelegationView {
        id_b64: delegation.id.to_base64(),
        delegation: delegation_json,
        created_at: record.created_at,
        revoked_at: record.revoked_at,
        revocation_reason: record.revocation_reason,
    };
    (StatusCode::CREATED, Json(view)).into_response()
}

#[derive(Debug, Default, Deserialize)]
pub struct ListDelegationParams {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ListDelegationsResponse {
    pub delegations: Vec<DelegationView>,
}

pub async fn list_delegations(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Query(params): Query<ListDelegationParams>,
) -> Response {
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    let records = match state.delegations.list_for_tenant(auth.tenant_id, limit, offset).await {
        Ok(r) => r,
        Err(e) => return internal_error(&e.to_string()),
    };

    let views: Vec<DelegationView> = records
        .into_iter()
        .map(|r| DelegationView {
            id_b64: URL_SAFE_NO_PAD.encode(&r.id),
            delegation: r.delegation_json,
            created_at: r.created_at,
            revoked_at: r.revoked_at,
            revocation_reason: r.revocation_reason,
        })
        .collect();

    (StatusCode::OK, Json(ListDelegationsResponse { delegations: views })).into_response()
}

pub async fn get_delegation(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_b64): Path<String>,
) -> Response {
    let id_bytes = match URL_SAFE_NO_PAD.decode(&id_b64) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => return bad_request("delegation id must decode to 32 bytes"),
        Err(_) => return bad_request("delegation id must be base64url-encoded"),
    };

    match state.delegations.get(auth.tenant_id, &id_bytes).await {
        Ok(record) => {
            let view = DelegationView {
                id_b64,
                delegation: record.delegation_json,
                created_at: record.created_at,
                revoked_at: record.revoked_at,
                revocation_reason: record.revocation_reason,
            };
            (StatusCode::OK, Json(view)).into_response()
        }
        Err(ConfigError::NotFound) => not_found("delegation not found"),
        Err(e) => internal_error(&e.to_string()),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct RevokeRequest {
    pub reason: Option<String>,
}

pub async fn revoke_delegation(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_b64): Path<String>,
    Json(req): Json<RevokeRequest>,
) -> Response {
    if let Err(r) = require_admin(&auth) {
        return r;
    }

    let id_bytes = match URL_SAFE_NO_PAD.decode(&id_b64) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => return bad_request("delegation id must decode to 32 bytes"),
        Err(_) => return bad_request("delegation id must be base64url-encoded"),
    };

    // Confirm the delegation exists in this tenant before doing anything.
    if let Err(ConfigError::NotFound) = state.delegations.get(auth.tenant_id, &id_bytes).await {
        return not_found("delegation not found");
    }

    // Mark it revoked in the database (the system of record).
    match state
        .delegations
        .mark_revoked(auth.tenant_id, &id_bytes, req.reason.as_deref())
        .await
    {
        Ok(()) => {
            // Publish to Redis so all gateway instances learn about it.
            // If publishing fails, we surface a 5xx — the database write
            // already happened, but we cannot promise propagation.
            let event = maat_config::RevocationEvent {
                tenant_id: auth.tenant_id,
                delegation_id_b64: id_b64.clone(),
                reason: req.reason.clone(),
                revoked_at: chrono::Utc::now(),
            };
            if let Err(e) = state.revocations.publish(&event).await {
                tracing::error!(
                    delegation = %id_b64,
                    error = %e,
                    "failed to publish revocation; running gateways may not see it until restart"
                );
                return internal_error(&format!("revocation recorded but not propagated: {}", e));
            }

            // Hint to keep the unused ObjectId import quiet.
            let _ = ObjectId([0u8; 32]);
            (StatusCode::NO_CONTENT).into_response()
        }
        Err(ConfigError::NotFound) => not_found("delegation not found or already revoked"),
        Err(e) => internal_error(&e.to_string()),
    }
}

// ─── Ledger summary (Slice 7) ───────────────────────────────────────────────

pub async fn get_ledger(
    State(state): State<Arc<DashboardState>>,
    Extension(auth): Extension<AuthContext>,
    Path(id_b64): Path<String>,
) -> Response {
    let id_bytes = match URL_SAFE_NO_PAD.decode(&id_b64) {
        Ok(b) if b.len() == 32 => b,
        Ok(_) => return bad_request("delegation id must decode to 32 bytes"),
        Err(_) => return bad_request("delegation id must be base64url-encoded"),
    };

    // Cross-tenant guard: confirm the delegation belongs to this tenant.
    if let Err(ConfigError::NotFound) =
        state.delegations.get(auth.tenant_id, &id_bytes).await
    {
        return not_found("delegation not found");
    }

    match crate::ledger::summary(&state.pool, auth.tenant_id, &id_bytes, 50).await {
        Ok(s) => (StatusCode::OK, Json(s)).into_response(),
        Err(e) => internal_error(&e.to_string()),
    }
}
