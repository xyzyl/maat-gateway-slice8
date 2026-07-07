# Maat Gateway

Multi-tenant verification gateway for the Maat protocol: verify/receipt
API, dashboard + UI, KMS, revocation propagation, and the cumulative
spend ledger. For the guided end-to-end demo loop, see
[demo/README.md](demo/README.md).

The notes below document the most recent protocol-affecting slice
(Slice 7 — constraint enforcement).

Constraints — the substantive limits a principal places on an agent's
authority — are now actively enforced. Per-action `MaxValue` works
through the existing protocol (`maat::ValueClaim` added). Cumulative
caps work through a Redis-backed gateway ledger that all gateway
instances share. The dashboard's create-delegation form has typed
constraint inputs; the detail view shows the running ledger; receipts
show the value claim that was checked.

## What changed since Slice 6

**Maat library (additive):**
- `ValueClaim { currency, amount, decimals }` added to `types.rs`.
- `ActionRequest` gains `value_claim: Option<ValueClaim>`.
- `verify_action_request` now enforces `MaxValue` against the claim
  with strict currency/decimals/amount comparison and fail-closed
  semantics on missing claim.
- Existing test vectors and tests untouched. Existing ActionRequest
  literals updated mechanically with `value_claim: None`.

**Gateway:**
- New `crates/gateway/src/ledger.rs` — Redis Lua atomic
  check-and-increment + Postgres durability.
- New migration `0003_ledger.sql` — `ledger_entries` (append-only
  via trigger) and `delegation_cumulative_caps`.
- Verify handler: revoke check → ledger reserve → protocol verify
  → on success: ledger commit; on failure: ledger release.
- The receipt's `action.value` bytes now contain the JSON-serialized
  `ValueClaim` for value-bearing actions, giving auditors a
  verifiable record.

**Dashboard service:**
- `POST /dashboard/v1/delegations` accepts optional `constraints` and
  `cumulative_cap`.
- `GET /dashboard/v1/delegations/:id/ledger` returns cap + recorded
  total + recent entries.

**Dashboard UI:**
- `ConstraintBuilder` component with typed inputs for `max_value`,
  `max_rate`, `domain_allow`, `require_anchor_freshness` plus the
  cumulative cap.
- `LedgerPanel` in the delegation detail modal (progress bar, recent
  activity).
- Receipts page decodes the value claim from each receipt and shows
  it inline.

## Running

You need everything from Slice 6 running, plus the new migration:

```sh
psql maat_gateway -f crates/gateway/migrations/0003_ledger.sql
```

Then start the same services as before.

## Demonstrating it

Through the UI:

1. Sign in.
2. **Create a principal key** (if you don't already have one).
3. **Create a delegation.** In the form, add a `max_value` constraint
   for $50.00 (5000 minor units, 2 decimals, USD), and enable the
   cumulative cap at $200.00 (20000, 2, USD).
4. The dashboard returns the signed delegation. Save the delegation
   JSON (paste from the detail modal into `delegation.json`).
5. **Submit verify requests through the gateway.** Each one needs a
   `value_claim`:

```powershell
$body = @{
  delegation_chain = @($delegationJson)
  anchor = $anchorJson
  action_scope = "commerce:purchase:execute"
  value_claim = @{ currency = "USD"; amount = 4000; decimals = 2 }
} | ConvertTo-Json -Depth 32

Invoke-RestMethod -Uri "http://localhost:8080/v1/verify" `
  -Method POST -ContentType "application/json" `
  -Headers @{ "Authorization" = "Bearer $apiKey" } -Body $body
```

   Five $40 actions succeed. The sixth — which would push the running
   total from $200 to $240 — is rejected with
   `reason: "constraint_violated"` and `detail` mentioning the
   cumulative cap.

6. **Check the ledger** in the dashboard UI: open the delegation
   detail modal. The Ledger panel shows the cap, the running total,
   and the recent entries.

## Tests

```sh
# Maat library
cargo test --manifest-path ~/code/maat/Cargo.toml

# Gateway workspace (needs Postgres + Redis)
$env:MAAT_GATEWAY_TEST_DATABASE_URL = "postgres://localhost/maat_gateway_test"
$env:MAAT_REDIS_TEST_URL = "redis://127.0.0.1:6379/"
cargo test --workspace
```

The Slice 7 tests are in:
- `~/code/maat/tests/value_claim_tests.rs` — protocol-level
  `MaxValue` + `ValueClaim` enforcement (7 tests).
- `crates/dashboard/tests/ledger_enforcement.rs` — cross-instance
  cumulative cap, per-action enforcement, missing-claim fail-closed,
  ledger summary endpoint.

## Failure modes

- **Redis unreachable mid-run on the gateway**: ledger reservation
  fails, verify returns 5xx. The protocol's per-action `MaxValue`
  still works (it doesn't touch Redis).
- **Postgres unreachable for ledger commit after a successful verify**:
  the receipt is signed and persisted; the durable ledger entry is
  not. Logged. Redis state is authoritative for subsequent decisions.
  Out-of-band reconciliation can replay missing rows from receipts.
- **Currency or decimals mismatch between claim and cap**: rejected
  at the gateway with `reason: "constraint_violated"`, no ledger
  state changes.

## Architectural notes

The cumulative cap is stored *outside* the signed delegation. This is
deliberate — the protocol-level `MaxValue` is what gets signed, and
the gateway-side cap is a layered cumulative limit on top. Auditors
reading a receipt see the per-action `MaxValue` from the delegation
plus, in the receipt's action bytes, the value claim that was
approved. The cumulative cap and running ledger are operational data
queryable through the dashboard, not part of the cryptographic record
itself.

A future protocol revision could promote cumulative caps to a
first-class signed constraint. That would be a wire-format change and
requires a 0.2.0 protocol bump. For Slice 7 the layered approach is
honest about which layer enforces what, and ships without disturbing
the existing test vectors.

## What this slice unlocks

Tier 2 use cases from the constraint vocabulary document — spend-
controlled shopping agents, rate-limited communication agents,
resource-bounded compute agents — are now claimable as "shipping
today." The headline demo (the $200 grocery budget agent that visibly
gets stopped at the cap) is buildable on top of this slice plus the
SDK that comes next.

## License

MIT OR Apache-2.0.
