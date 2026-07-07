# Maat Demo Loop

A real autonomous agent buying real (test-mode Stripe) goods at a real
e-commerce service, with the Maat protocol enforcing the trust
boundary between them.

This is the demo that makes the project legible. Three browser tabs,
one terminal for service logs, ten steps from cold open to the agent
visibly hitting its budget cap and stopping.

---

## What runs during the demo

Six processes:

| What | Port | Purpose |
|---|---|---|
| KMS service | 9090 | Holds the gateway's executor signing key. |
| Gateway | 8080 | Issues receipts; enforces protocol-level constraints + cumulative ledger. |
| Dashboard service | 8081 | Tenant management, delegation creation. |
| Dashboard UI (Vite) | 5173 | The operator's view. |
| Demo store | 8090 | The merchant. Verifies receipts, charges Stripe, records transactions. |
| Demo agent | 9000 | The autonomous shopper. Generates its own keypair, accepts a delegation, runs a goal. |

The agent and the store both depend only on the `maat` library. The
store does not depend on the gateway or dashboard internals — it's
written the way an external integrator would write it.

---

## Pre-requisites

- Postgres + Redis running locally
- Two databases created and migrated:
  ```
  createdb maat_gateway       # gateway + dashboard share this database
  createdb maat_demo_store    # demo store has its own
  ```
  Apply migrations from `crates/config/migrations`, `crates/gateway/migrations`,
  and `crates/demo-store/migrations` (in that order).
- A Stripe test mode account. Get the secret key from
  https://dashboard.stripe.com/test/apikeys — it starts with `sk_test_`.

---

## One-time setup

The setup script needs the KMS, gateway, and dashboard already running.
See "Starting the services" below for that, then come back.

From a PowerShell or bash prompt at the repo root:

```powershell
$env:STRIPE_SECRET_KEY    = "sk_test_..."
$env:DATABASE_URL         = "postgres://localhost/maat_gateway"
$env:MAAT_KMS_AUTH_TOKEN  = "<same token used to start the KMS>"
.\demo\setup.ps1
```

```bash
export STRIPE_SECRET_KEY="sk_test_..."
export DATABASE_URL="postgres://localhost/maat_gateway"
export MAAT_KMS_AUTH_TOKEN="<same token used to start the KMS>"
./demo/setup.sh
```

This:
1. Creates a Maat tenant + admin user (`admin@maat-demo.local` /
   `demo-password`) and prints the tenant API key.
2. Creates a Stripe test customer with the always-succeed test card
   (`tok_visa` → 4242 4242 4242 4242) attached as the default payment
   method.
3. Seeds the demo store's products and customer rows.

The script prints the values you'll need as environment variables.
**Save the output** — particularly `MAAT_API_KEY`, `STRIPE_CUSTOMER_ID`,
and `DEMO_AGENT_CUSTOMER_ID`.

---

## Starting the services

Open six terminals (or panes). Each command runs from the repo root.

**Generate the KMS auth token** (one-time, before starting any service):
```
cargo run -p maat-kms --bin maat-kmsd -- generate-auth-token
```
Save the output. Set it as `MAAT_KMS_AUTH_TOKEN` in **every** service that
talks to the KMS — the KMS itself, the gateway, the dashboard, the
bootstrap binary, and the setup script. The store and agent do **not**
need it (they only talk to the gateway).

**KMS** (terminal 1):
```
$env:MAAT_KMS_VAULT      = ".\kms-vault"
$env:MAAT_KMS_MASTER_KEY = "<32-byte base64url; see crates/kms/README.md>"
$env:MAAT_KMS_AUTH_TOKEN = "<from generate-auth-token>"
cargo run -p maat-kms --bin maat-kmsd
```

**Gateway** (terminal 2):
```
$env:DATABASE_URL        = "postgres://localhost/maat_gateway"
$env:MAAT_KMS_URL        = "http://127.0.0.1:9090"
$env:MAAT_KMS_AUTH_TOKEN = "<same token>"
$env:MAAT_REDIS_URL      = "redis://127.0.0.1:6379/"
cargo run -p maat-gateway --bin maat-gatewayd
```

**Dashboard service** (terminal 3):
```
$env:DATABASE_URL        = "postgres://localhost/maat_gateway"
$env:MAAT_KMS_URL        = "http://127.0.0.1:9090"
$env:MAAT_KMS_AUTH_TOKEN = "<same token>"
$env:MAAT_REDIS_URL      = "redis://127.0.0.1:6379/"
cargo run -p maat-dashboard --bin maat-dashboardd
```

**Dashboard UI** (terminal 4):
```
cd crates/dashboard-ui
npm install
npm run dev
```

**Demo store** (terminal 5):
```
$env:DEMO_STORE_DATABASE_URL = "postgres://localhost/maat_demo_store"
$env:STRIPE_SECRET_KEY = "sk_test_..."
$env:MAAT_GATEWAY_URL = "http://127.0.0.1:8080"
$env:MAAT_API_KEY = "<from setup output>"
cargo run -p demo-store --bin demo-stored
```

**Demo agent** (terminal 6):
```
$env:MAAT_API_KEY = "<from setup output>"
$env:DEMO_AGENT_CUSTOMER_ID = "<from setup output>"
cargo run -p demo-agent --bin demo-agent
```

---

## The 10-step demo flow

Open three browser tabs:
- **Dashboard:** http://localhost:5173
- **Store:** http://localhost:8090
- **Agent:** http://localhost:9000

Then:

1. **Sign into the dashboard.** Email `admin@maat-demo.local`,
   password `demo-password`, tenant `maat-demo`. The dashboard shows
   an empty "Delegations" list.

2. **Create a principal key.** Navigate to *Principal Keys* → "New
   key". Name it `demo`. The dashboard reveals an API key (you can
   ignore it — you already have one from setup).

3. **Look at the agent.** The agent tab shows phase = `awaiting`
   and a public key. **Click "Copy public key".**

4. **Create the delegation.** Back in the dashboard, *Delegations*
   → "New delegation". Paste the agent's public key. Set the scopes
   field to `commerce:purchase:execute, commerce:catalog:read`. Set
   expiry to 1 hour.

   In the constraint builder, add:
   - `max_value` constraint: $20.00 (currency `USD`, amount 2000,
     decimals 2)
   - `max_rate`: 10 actions / 60 seconds
   - Cumulative cap: $50.00 (USD, amount 5000, decimals 2)

   Click *Create*. The dashboard returns a signed delegation JSON in
   a modal.

5. **Activate the agent.** Copy the entire delegation JSON. Paste
   into the agent's *Delegation* textarea. Click *Validate &
   activate*. The agent's phase changes to `ready`.

6. **Submit a goal.** In the agent's *Goal* panel:
   - Max total spend: 4000 (¢40.00)
   - Max per item: 2000 (¢20.00)
   - Mode: `happy path`

   Click *Run*.

7. **Watch the loop close.** The agent's activity log streams in real
   time:
   - "fetched catalog: 12 items"
   - "picked 'Ripe bananas, 1lb' (199¢), claiming 199¢"
   - "gateway approved 'Ripe bananas, 1lb' for 199¢"
   - "store charged 199¢ via stripe (pi_…)"
   - …

   Simultaneously, the store tab shows transactions appearing.

8. **Hit the cumulative cap.** Submit a new goal with max total =
   8000 (¢80.00). The agent now has $50 of headroom. It will run
   through items until the next purchase would push past $50, at
   which point the gateway rejects with `cumulative cap exceeded`
   and the agent stops. The store sees no further transactions.

9. **Force a per-action rejection.** Submit a goal with mode =
   `force per-action cap rejection`. The agent claims more than the
   item costs (more than $20 per claim). The gateway rejects with
   `max_value`. The store sees nothing.

10. **Revoke mid-loop.** Start a happy-path run, then in the
    dashboard click *Revoke* on the delegation. The next gateway
    call fails with `revoked`; the agent stops. (Slice 5's
    cross-instance revocation propagation makes this work even if
    the agent was about to send a request.)

---

## What you just watched

The protocol enforced two trust boundaries at once:

**Agent ⇄ gateway.** The agent submitted signed claims; the gateway
verified the delegation chain, the agent's signature on the anchor,
the constraint set, and the cumulative ledger before issuing a
receipt.

**Resource ⇄ gateway.** The store independently verified the receipt
signature against the gateway's executor public key (fetched once at
startup) and matched the receipt's value claim to the cart total to
the cent. A receipt for a different amount would have been rejected
by the store regardless of how cryptographically valid it was.

Real Stripe API calls happened in test mode. The store's transaction
table holds real `pi_test_...` payment intent IDs you can verify in
the Stripe dashboard.

---

## What's next

Slice 9 will add the registration-request pattern: the agent posts
its public key to a `pending_agents` endpoint, the dashboard shows a
"Pending registrations" section, the operator approves with one
click and the delegation is pushed to the agent automatically. Same
trust boundary, fewer copy-paste steps.

Beyond that, the SDK slice — extracting the agent's verify-then-
checkout loop into a published Rust crate (and a TypeScript port)
that external integrators can drop into their own agents.

Switching this demo from test-mode Stripe to live mode is a one-line
config change.
