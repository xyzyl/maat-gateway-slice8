//! Failure-receipt emission.

use maat::{
    Action, Delegation, MaatError, ObjectId, Outcome, Receipt, Signer,
};
use tracing::warn;

/// Mint a Failure receipt attributing `error` to the leaf delegation
/// in the request. The receipt is signed by the executor.
///
/// Generic over `Signer` rather than `&dyn Signer` because `Receipt::new`
/// requires `S: Signer` (Sized). Callers pass any concrete signer:
/// `mint_failure_receipt(signer.as_ref(), ...)` where signer is `Arc<KmsClient>`.
// One parameter per receipt ingredient; a params struct would obscure the
// single call site in the verify handler.
#[allow(clippy::too_many_arguments)]
pub fn mint_failure_receipt<S: Signer>(
    executor: &S,
    leaf_delegation: &Delegation,
    anchor_id: ObjectId,
    action_scope: &str,
    action_description: Option<&str>,
    action_value: Vec<u8>,
    error: &MaatError,
    delegation_chain_ids: Vec<ObjectId>,
) -> Result<Receipt, MaatError> {
    let action = Action {
        scope_used: action_scope.into(),
        description: action_description.unwrap_or_default().into(),
        value: action_value,
    };
    let (outcome_code, detail) = classify_error(error);
    Receipt::new(
        executor,
        leaf_delegation.id.clone(),
        anchor_id,
        action,
        Outcome::Failure,
        Some(format!("{}: {}", outcome_code, detail)),
        delegation_chain_ids,
    )
}

/// Map a MaatError to a short stable code + human-readable detail.
pub fn classify_error(e: &MaatError) -> (&'static str, String) {
    match e {
        MaatError::Revoked => ("revoked", "delegation revoked".into()),
        MaatError::DelegationExpired => ("expired", "delegation expired".into()),
        MaatError::DelegationNotYetActive => {
            ("not_yet_active", "delegation not_before in future".into())
        }
        MaatError::InvalidSignature => {
            ("invalid_signature", "signature verification failed".into())
        }
        MaatError::AnchorInFuture => ("anchor_future", "anchor created_at in future".into()),
        MaatError::AnchorTooStale => ("anchor_stale", "anchor exceeded max_staleness".into()),
        MaatError::AnchorAgentMismatch => {
            ("anchor_agent_mismatch", "anchor signed by wrong agent".into())
        }
        MaatError::AnchorDelegationNotFound => (
            "anchor_delegation_mismatch",
            "anchor references different delegation".into(),
        ),
        MaatError::ChainMismatch => (
            "chain_mismatch",
            "delegation chain principal/agent mismatch".into(),
        ),
        MaatError::ScopeNotContained => {
            ("scope_attenuation", "child scope not contained by parent".into())
        }
        MaatError::ActionScopeNotContained => {
            ("scope_violated", "action scope not permitted by delegation".into())
        }
        MaatError::AnchorFreshnessExceedsConstraint => (
            "anchor_freshness_exceeded",
            "anchor max_staleness exceeds RequireAnchorFreshness constraint".into(),
        ),
        MaatError::ReplayDetected => ("replay", "nonce pair already seen".into()),
        MaatError::SelfAttestation => {
            ("self_attestation", "executor cannot be any agent in chain".into())
        }
        MaatError::ConstraintViolated(msg) => ("constraint_violated", msg.clone()),
        MaatError::Crypto(msg) => ("crypto", msg.clone()),
        other => {
            warn!("unmapped MaatError in failure receipt classifier: {:?}", other);
            ("other", format!("{:?}", other))
        }
    }
}
