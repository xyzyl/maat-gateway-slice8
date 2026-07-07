#!/usr/bin/env bash
# Slice 8 demo setup (bash)
set -euo pipefail

TENANT_SLUG="${TENANT_SLUG:-maat-demo}"
TENANT_NAME="${TENANT_NAME:-Maat Demo}"
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@maat-demo.local}"
ADMIN_PASSWORD="${ADMIN_PASSWORD:-demo-password}"
API_KEY_NAME="${API_KEY_NAME:-demo-key}"
DEMO_STORE_DB="${DEMO_STORE_DB:-postgres://localhost/maat_demo_store}"
DEMO_STORE_URL="${DEMO_STORE_URL:-http://127.0.0.1:8090}"
CUSTOMER_EMAIL="${CUSTOMER_EMAIL:-demo-shopper@maat-demo.local}"

if [[ -z "${STRIPE_SECRET_KEY:-}" ]]; then
    echo "STRIPE_SECRET_KEY env var must be set (use a sk_test_... key)" >&2
    exit 1
fi
case "$STRIPE_SECRET_KEY" in
    sk_test_*) ;;
    *) echo "Warning: STRIPE_SECRET_KEY is not a test key" >&2 ;;
esac
if [[ -z "${DATABASE_URL:-}" ]] && [[ -z "${MAAT_CONFIG_DATABASE_URL:-}" ]]; then
    echo "Set DATABASE_URL (or MAAT_CONFIG_DATABASE_URL) to the gateway DB before running setup." >&2
    exit 1
fi
if [[ -z "${MAAT_KMS_AUTH_TOKEN:-}" ]]; then
    echo "MAAT_KMS_AUTH_TOKEN must be set (matching the KMS's value). Generate one with:" >&2
    echo "  cargo run -p maat-kms --bin maat-kmsd -- generate-auth-token" >&2
    exit 1
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE="$(dirname "$HERE")"

run_bootstrap() {
    cd "$WORKSPACE"
    cargo run -q -p maat-gateway --bin maat-gateway-bootstrap -- "$@" 2>&1
}

echo "Provisioning Maat tenant..."
run_bootstrap create-tenant "$TENANT_SLUG" "$TENANT_NAME" || true

echo "Provisioning API key..."
KEY_OUT="$(run_bootstrap create-api-key "$TENANT_SLUG" "$API_KEY_NAME")"
API_KEY="$(echo "$KEY_OUT" | grep -oE '^\s*mgw_live_[A-Za-z0-9_-]{20,}\s*$' | head -1 | tr -d '[:space:]')"
if [[ -z "$API_KEY" ]]; then
    echo "----- bootstrap output -----" >&2
    echo "$KEY_OUT" >&2
    echo "----------------------------" >&2
    echo "Could not extract API key from bootstrap output." >&2
    exit 1
fi
echo "  api_key   = $API_KEY"

echo "Provisioning admin user..."
run_bootstrap create-user "$TENANT_SLUG" "$ADMIN_EMAIL" "$ADMIN_PASSWORD" admin > /dev/null

echo
echo "Creating Stripe test customer with attached test card..."
STRIPE_BASE="https://api.stripe.com/v1"

CUST_JSON="$(curl -s -u "$STRIPE_SECRET_KEY:" "$STRIPE_BASE/customers" \
    -d "email=$CUSTOMER_EMAIL" -d "name=Demo Shopper")"
STRIPE_CUSTOMER_ID="$(echo "$CUST_JSON" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')"
echo "  stripe customer = $STRIPE_CUSTOMER_ID"

PM_JSON="$(curl -s -u "$STRIPE_SECRET_KEY:" "$STRIPE_BASE/payment_methods" \
    -d "type=card" -d "card[token]=tok_visa")"
PM_ID="$(echo "$PM_JSON" | python3 -c 'import sys,json; print(json.load(sys.stdin)["id"])')"
echo "  payment method  = $PM_ID"

curl -s -u "$STRIPE_SECRET_KEY:" "$STRIPE_BASE/payment_methods/$PM_ID/attach" \
    -d "customer=$STRIPE_CUSTOMER_ID" >/dev/null
curl -s -u "$STRIPE_SECRET_KEY:" -X POST "$STRIPE_BASE/customers/$STRIPE_CUSTOMER_ID" \
    -d "invoice_settings[default_payment_method]=$PM_ID" >/dev/null

echo
echo "Seeding demo store with products..."
SEED_FILE="$HERE/seed-products.json"
python3 - <<EOF
import json, subprocess
products = json.load(open("$SEED_FILE"))
for p in products:
    img = "'" + p['image_url'].replace("'", "''") + "'" if p.get('image_url') else 'NULL'
    sku = p['sku'].replace("'", "''")
    name = p['name'].replace("'", "''")
    desc = p['description'].replace("'", "''")
    cur = p['currency'].replace("'", "''")
    sql = (
        f"INSERT INTO products (sku, name, description, price_cents, currency, image_url) "
        f"VALUES ('{sku}', '{name}', '{desc}', {p['price_cents']}, '{cur}', {img}) "
        f"ON CONFLICT (sku) DO UPDATE SET "
        f"name = EXCLUDED.name, description = EXCLUDED.description, "
        f"price_cents = EXCLUDED.price_cents, currency = EXCLUDED.currency, "
        f"image_url = EXCLUDED.image_url;"
    )
    subprocess.run(["psql", "$DEMO_STORE_DB", "-c", sql], check=True, capture_output=True)
print(f"  {len(products)} products seeded")
EOF

DEMO_CUSTOMER_ID="$(psql "$DEMO_STORE_DB" -t -A -c "
INSERT INTO customers (email, stripe_customer_id) VALUES ('$CUSTOMER_EMAIL', '$STRIPE_CUSTOMER_ID')
ON CONFLICT (email) DO UPDATE SET stripe_customer_id = EXCLUDED.stripe_customer_id
RETURNING id;
" | tr -d '[:space:]')"
echo "  demo customer id = $DEMO_CUSTOMER_ID"

echo
echo "Done."
echo
echo "Save these for the demo:"
echo "  TENANT_SLUG            = $TENANT_SLUG"
echo "  ADMIN_EMAIL            = $ADMIN_EMAIL"
echo "  ADMIN_PASSWORD         = $ADMIN_PASSWORD"
echo "  MAAT_API_KEY           = $API_KEY"
echo "  STRIPE_CUSTOMER_ID     = $STRIPE_CUSTOMER_ID"
echo "  DEMO_AGENT_CUSTOMER_ID = $DEMO_CUSTOMER_ID"
