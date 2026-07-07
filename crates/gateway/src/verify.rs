//! POST /v1/verify handler. 0.4.0 production version.

use std::sync::Arc;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use maat::verify::{verify_action_request_with, ActionRequest};
use maat::{Action, Anchor, Delegation, MaatError, Outcome, Receipt};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

use crate::auth::TenantContext;
use crate::failure_receipt::{classify_error, mint_failure_receipt};
use crate::human_confirm::{PendingConfirmEvaluator, PENDING_SENTINEL};
use crate::state::GatewayState;
use crate::storage::rate_redis::RateStoreAdapter;
use crate::storage::replay_redis::ReplayStoreAdapter;

#[derive(Debug, Deserialize)]
pub struct VerifyRequest {
    pub delegation_chain: Vec<Delegation>,
    pub anchor: Anchor,
    pub action_scope: String,
    #[serde(default)]
    pub domain: Option<String>,
    #[serde(default)]
    pub action_description: Option<String>,
    #[serde(default)]
    pub value_claim: Option<maat::ValueClaim>,
    #[serde(default)]
    pub action_value: Option<Vec<u8>>,
}

#[derive(Debug, Serialize)]
pub struct VerifySuccessResponse {
    pub verified: bool,
    pub receipt: Receipt,
    pub delegation_id: String,
    pub verification_time_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct VerifyRejectionResponse {
    pub verified: bool,
    pub receipt: Receipt,
    pub reason: String,
    pub detail: String,
    pub failed_check: String,
}

#[derive(Debug, Serialize)]
pub struct PendingConfirmResponse {
    pub verified: bool,
    pub pending_id: uuid::Uuid,
    pub status: &'static str,
    pub detail: &'static str,
}

pub async fn handler(
    State(state): State<Arc<GatewayState>>,
    Extension(tenant): Extension<TenantContext>,
    Json(req): Json<VerifyRequest>,
) -> Response {
    let start = std::time::Instant::now();

    if req.delegation_chain.is_empty() {
        return bad_request("empty delegation_chain");
    }

    let signer: Arc<crate::kms_client::KmsClient> =
        match state.signer_for(&tenant.kms_executor_key_id).await {
            Ok(s) => s,
            Err(e) => {
                error!(tenant_id = %tenant.tenant_id, "signer resolution failed: {}", e);
                return internal_error("signer unavailable");
            }
        };

    let leaf_delegation = req.delegation_chain.last().unwrap().clone();
    let delegation_id_b64 = leaf_delegation.id.to_base64();
    let anchor_nonce = req.anchor.nonce.0;
    let anchor_id = req.anchor.id.clone();

    let action_value: Vec<u8> = req
        .action_value
        .clone()
        .or_else(|| {
            req.value_claim
                .as_ref()
                .and_then(|c| serde_json::to_vec(c).ok())
        })
        .unwrap_or_default();

    let action_request = ActionRequest {
        delegation: leaf_delegation.clone(),
        delegation_chain: req.delegation_chain.clone(),
        anchor: req.anchor.clone(),
        action_scope: req.action_scope.clone(),
        domain: req.domain.clone(),
        value_claim: req.value_claim.clone(),
        action_value: req.action_value.clone(),
    };

    let current_time = match maat::types::now() {
        Ok(t) => t,
        Err(e) => {
            error!("system clock error: {}", e);
            return internal_error("system clock error");
        }
    };

    // ── Ledger reserve (Slice 7 path, unchanged) ──
    // Reservation: !Clone. Move (don't clone) into the rollback / commit
    // branches below.
    let mut reservation: Option<crate::ledger::Reservation> = None;
    let ledger_pre_check_err: Option<MaatError> = if let Some(claim) = &req.value_claim {
        match state
            .ledger
            .check_and_reserve(
                tenant.tenant_id,
                &leaf_delegation.id.0,
                &claim.currency,
                claim.amount,
                claim.decimals,
            )
            .await
        {
            Ok(r) => {
                reservation = r;
                None
            }
            Err(crate::ledger::LedgerError::CapExceeded { would_be, cap }) => Some(
                MaatError::ConstraintViolated(format!(
                    "cumulative cap exceeded: would be {}, cap {}",
                    would_be, cap
                )),
            ),
            Err(e) => {
                error!("ledger reserve error: {}", e);
                return internal_error("ledger error");
            }
        }
    } else {
        None
    };

    // ── Chain revocation: consult RevocationCache for EVERY delegation ──
    let revocations = state.revocations.clone();
    let is_revoked = move |id: &maat::ObjectId| revocations.is_revoked(&id.0);

    // ── Distributed replay + rate stores ──
    let mut replay_adapter = ReplayStoreAdapter::new(&state.replay_store, current_time);
    let mut rate_adapter = RateStoreAdapter::new(&state.rate_store);

    // ── Custom-constraint registry ──
    let custom_eval = state.custom_registry.clone();

    // ── Human-confirm evaluator backed by pending_confirms table ──
    let human_eval = PendingConfirmEvaluator {
        repo: &state.pending_confirms,
        tenant_id: tenant.tenant_id,
        anchor_nonce,
    };

    // ── Verify ──
    let verification_result: Result<(), MaatError> = if let Some(e) = ledger_pre_check_err {
        Err(e)
    } else {
        verify_action_request_with(
            &action_request,
            current_time,
            &is_revoked,
            &mut replay_adapter,
            &mut rate_adapter,
            &custom_eval,
            &human_eval,
        )
    };

    // ── Roll back ledger reservation on failure ──
    if verification_result.is_err() {
        if let Some(ref r) = reservation {
            state.ledger.release(r).await;
            reservation = None;
        }
    }

    let elapsed_ms = start.elapsed().as_millis();

    match verification_result {
        Ok(()) => {
            // Mint success receipt FIRST (its id is needed by the ledger commit).
            let action = Action {
                scope_used: req.action_scope.clone(),
                description: req.action_description.clone().unwrap_or_default(),
                value: action_value.clone(),
            };
            let chain_ids: Vec<_> =
                req.delegation_chain.iter().map(|d| d.id.clone()).collect();
            let receipt = match Receipt::new(
                signer.as_ref(),
                leaf_delegation.id.clone(),
                anchor_id,
                action,
                Outcome::Success,
                None,
                chain_ids,
            ) {
                Ok(r) => r,
                Err(e) => {
                    error!("receipt mint failed: {}", e);
                    if let Some(ref r) = reservation {
                        state.ledger.release(r).await;
                    }
                    return internal_error("receipt mint failed");
                }
            };

            // Commit the ledger reservation now that we have a receipt id.
            if let Some(r) = reservation.as_ref() {
                if let Some(claim) = &req.value_claim {
                    if let Err(e) = state
                        .ledger
                        .commit(
                            tenant.tenant_id,
                            &leaf_delegation.id.0,
                            &receipt.id.0,
                            &claim.currency,
                            r.reserved_amount,
                            claim.decimals,
                        )
                        .await
                    {
                        warn!(error = %e, "ledger commit failed; redis state remains authoritative");
                    }
                }
            }

            // Consume an approved pending-confirm if any (no-op otherwise).
            let _ = state
                .pending_confirms
                .consume(tenant.tenant_id, &leaf_delegation.id.0, &anchor_nonce)
                .await;

            // Store receipt.
            if let Err(e) = state.store.write(tenant.tenant_id, &receipt).await {
                error!("receipt store write failed: {}", e);
                warn!("returning success without durable receipt storage");
            }

            info!(
                tenant_id = %tenant.tenant_id,
                delegation_id = %delegation_id_b64,
                elapsed_ms,
                "verify ok"
            );
            (
                StatusCode::OK,
                Json(VerifySuccessResponse {
                    verified: true,
                    receipt,
                    delegation_id: delegation_id_b64,
                    verification_time_ms: elapsed_ms,
                }),
            )
                .into_response()
        }

        Err(e) => {
            // Detect the pending-confirm sentinel: HTTP 202 path.
            if let MaatError::ConstraintViolated(ref msg) = e {
                if msg.contains(PENDING_SENTINEL) {
                    let pending = state
                        .pending_confirms
                        .insert_pending(
                            tenant.tenant_id,
                            &leaf_delegation.id.0,
                            &anchor_nonce,
                            &req.action_scope,
                            req.action_description.as_deref(),
                        )
                        .await;
                    match pending {
                        Ok(p) => {
                            return (
                                StatusCode::ACCEPTED,
                                Json(PendingConfirmResponse {
                                    verified: false,
                                    pending_id: p.id,
                                    status: "awaiting_human_confirm",
                                    detail: "submit again once approved via the dashboard",
                                }),
                            )
                                .into_response();
                        }
                        Err(err) => {
                            error!("pending_confirms insert failed: {}", err);
                            return internal_error("pending_confirms storage error");
                        }
                    }
                }
            }

            // Mint failure receipt.
            let (failed_check, detail) = classify_error(&e);
            let chain_ids: Vec<_> =
                req.delegation_chain.iter().map(|d| d.id.clone()).collect();
            let failure_receipt = match mint_failure_receipt(
                signer.as_ref(),
                &leaf_delegation,
                anchor_id,
                &req.action_scope,
                req.action_description.as_deref(),
                action_value,
                &e,
                chain_ids,
            ) {
                Ok(r) => r,
                Err(e) => {
                    error!("failure receipt mint failed: {}", e);
                    return internal_error("failure receipt mint failed");
                }
            };

            if let Err(e) = state.store.write(tenant.tenant_id, &failure_receipt).await {
                warn!("failure receipt store write failed: {}", e);
            }

            info!(
                tenant_id = %tenant.tenant_id,
                delegation_id = %delegation_id_b64,
                elapsed_ms,
                failed_check,
                "verify rejected"
            );

            let status = match failed_check {
                "invalid_signature" | "self_attestation" => StatusCode::UNAUTHORIZED,
                "replay" => StatusCode::CONFLICT,
                "crypto" | "other" => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::FORBIDDEN,
            };
            (
                status,
                Json(VerifyRejectionResponse {
                    verified: false,
                    receipt: failure_receipt,
                    reason: failed_check.to_string(),
                    detail,
                    failed_check: failed_check.to_string(),
                }),
            )
                .into_response()
        }
    }
}

fn bad_request(msg: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}

fn internal_error(msg: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": msg })),
    )
        .into_response()
}
