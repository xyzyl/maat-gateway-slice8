//! `HumanConfirmEvaluator` impl backed by the pending-confirms table.
//!
//! On verify, the evaluator checks for an approved entry matching
//! (tenant, delegation_id, anchor_nonce). If found, permits the
//! constraint. If not found OR still pending, returns a special
//! "pending-confirmation" error that the gateway translates into HTTP
//! 202 + the pending_id.

use maat::verify::{CustomConstraintContext, HumanConfirmEvaluator};
use maat::{MaatError, Result as MaatResult};
use uuid::Uuid;

use crate::pending_confirms::{ConfirmStatus, PendingConfirmRepo};

/// Sentinel marker in the error message that the gateway's response
/// path matches on to translate to HTTP 202.
pub const PENDING_SENTINEL: &str = "__maat_pending_human_confirm__";

pub struct PendingConfirmEvaluator<'a> {
    pub repo: &'a PendingConfirmRepo,
    pub tenant_id: Uuid,
    /// We capture the anchor nonce up front because the evaluator
    /// receives only the CustomConstraintContext which doesn't include it.
    pub anchor_nonce: [u8; 16],
}

impl<'a> HumanConfirmEvaluator for PendingConfirmEvaluator<'a> {
    fn evaluate(
        &self,
        _threshold: &str,
        ctx: &CustomConstraintContext,
    ) -> MaatResult<()> {
        let did = ctx.delegation.id.0;
        let nonce = self.anchor_nonce;
        let tenant = self.tenant_id;
        let repo = self.repo.clone();

        let entry = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current()
                .block_on(async move { repo.find(tenant, &did, &nonce).await })
        })
        .map_err(|e| MaatError::ConstraintViolated(format!("pending lookup: {}", e)))?;

        match entry {
            Some(e) if e.status == ConfirmStatus::Approved => Ok(()),
            Some(e) if e.status == ConfirmStatus::Denied => Err(MaatError::ConstraintViolated(
                "human confirmation was denied".into(),
            )),
            Some(e) if e.status == ConfirmStatus::Consumed => Err(MaatError::ConstraintViolated(
                "human confirmation already consumed".into(),
            )),
            _ => Err(MaatError::ConstraintViolated(format!(
                "{}: awaiting human confirmation",
                PENDING_SENTINEL
            ))),
        }
    }
}
