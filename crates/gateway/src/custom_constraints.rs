//! Gateway-side custom-constraint registry.
//!
//! Operators register evaluators for known custom constraint types at
//! gateway startup. The registry implements `CustomConstraintEvaluator`
//! and is passed into `verify_action_request_with` on every verify call.
//! Unknown type URIs fail closed (the registry returns
//! `ConstraintViolated` for any URI not in its map).
//!
//! Shipped reference evaluator: `OrderMaxAge` — the constraint says
//! "this delegation may only be used to act on orders no older than N
//! days." The evaluator parses the JSON-encoded `value` to get the max
//! age and consults `action_value` (a JSON object containing
//! `order_created_at: u64`) to verify.

use std::collections::HashMap;
use std::sync::Arc;

use maat::verify::{CustomConstraintContext, CustomConstraintEvaluator};
use maat::{MaatError, Result as MaatResult};
use serde::Deserialize;

/// A registered evaluator function.
type EvaluatorFn =
    dyn Fn(&[u8], &CustomConstraintContext) -> MaatResult<()> + Send + Sync + 'static;

/// Registry of custom-constraint evaluators. Construct at gateway
/// startup; pass to `verify_action_request_with` on every request.
#[derive(Clone)]
pub struct CustomConstraintRegistry {
    inner: Arc<HashMap<String, Arc<EvaluatorFn>>>,
}

impl CustomConstraintRegistry {
    pub fn builder() -> CustomConstraintRegistryBuilder {
        CustomConstraintRegistryBuilder {
            map: HashMap::new(),
        }
    }

    /// Empty registry — every Custom variant fails closed. Useful
    /// when no operator-side constraints are configured.
    pub fn empty() -> Self {
        Self {
            inner: Arc::new(HashMap::new()),
        }
    }
}

pub struct CustomConstraintRegistryBuilder {
    map: HashMap<String, Arc<EvaluatorFn>>,
}

impl CustomConstraintRegistryBuilder {
    pub fn register<F>(mut self, type_uri: impl Into<String>, evaluator: F) -> Self
    where
        F: Fn(&[u8], &CustomConstraintContext) -> MaatResult<()> + Send + Sync + 'static,
    {
        self.map
            .insert(type_uri.into(), Arc::new(evaluator) as Arc<EvaluatorFn>);
        self
    }

    pub fn build(self) -> CustomConstraintRegistry {
        CustomConstraintRegistry {
            inner: Arc::new(self.map),
        }
    }
}

impl CustomConstraintEvaluator for CustomConstraintRegistry {
    fn evaluate(
        &self,
        type_uri: &str,
        value: &[u8],
        ctx: &CustomConstraintContext,
    ) -> MaatResult<()> {
        match self.inner.get(type_uri) {
            Some(eval) => eval(value, ctx),
            None => Err(MaatError::ConstraintViolated(format!(
                "custom constraint '{}' is not registered with this gateway — fail closed",
                type_uri
            ))),
        }
    }
}

// ─── Reference evaluator: OrderMaxAge ──────────────────────────────────────

/// Reference custom-constraint evaluator. Demonstrates the pattern.
///
/// The constraint's `value` bytes are expected to be JSON of the form
/// `{"max_age_days": u32}`. The request's `action_value` must be JSON
/// containing `{"order_created_at": u64}` (Unix seconds). The evaluator
/// rejects if the order is older than the limit.
pub fn order_max_age_evaluator() -> impl Fn(&[u8], &CustomConstraintContext) -> MaatResult<()>
       + Send
       + Sync
       + 'static {
    |value, ctx| {
        #[derive(Deserialize)]
        struct ConstraintValue {
            max_age_days: u32,
        }
        #[derive(Deserialize)]
        struct ActionValue {
            order_created_at: u64,
        }

        let cv: ConstraintValue = serde_json::from_slice(value).map_err(|e| {
            MaatError::ConstraintViolated(format!(
                "order_max_age constraint value malformed: {}",
                e
            ))
        })?;

        let av_bytes = ctx.action_value.ok_or_else(|| {
            MaatError::ConstraintViolated(
                "order_max_age requires action_value with order_created_at".into(),
            )
        })?;
        let av: ActionValue = serde_json::from_slice(av_bytes).map_err(|e| {
            MaatError::ConstraintViolated(format!(
                "order_max_age action_value malformed: {}",
                e
            ))
        })?;

        let now = maat::types::now().unwrap_or(0);
        let max_age_secs = (cv.max_age_days as u64).saturating_mul(86_400);
        if now.saturating_sub(av.order_created_at) > max_age_secs {
            return Err(MaatError::ConstraintViolated(format!(
                "order age {}s exceeds max {}s",
                now.saturating_sub(av.order_created_at),
                max_age_secs
            )));
        }
        Ok(())
    }
}

/// Stable URI for the reference evaluator.
pub const ORDER_MAX_AGE_URI: &str = "maat.example/order_max_age/v1";

/// Convenience: a registry pre-populated with the reference evaluators.
pub fn default_registry() -> CustomConstraintRegistry {
    CustomConstraintRegistry::builder()
        .register(ORDER_MAX_AGE_URI, order_max_age_evaluator())
        .build()
}
