# Slice 8 demo setup (Windows / PowerShell)
#
# Provisions the out-of-band state for the demo loop:
#   - Maat tenant + admin user + API key (via maat-gateway-bootstrap)
#   - Stripe test customer with attached test card
#   - Sample products in the demo store
#
# Pre-requisites:
#   - Postgres running locally; databases 'maat_gateway' and 'maat_demo_store'
#     already created with their respective migrations applied
#   - Redis running locally
#   - $env:STRIPE_SECRET_KEY set to a test mode secret key (sk_test_...)
#   - $env:DATABASE_URL pointing at the gateway/dashboard DB
#     (e.g. postgres://localhost/maat_gateway)
#   - $env:MAAT_KMS_AUTH_TOKEN set to the same value the running KMS was
#     started with (generate one with `cargo run -p maat-kms --bin maat-kmsd
#     -- generate-auth-token` BEFORE starting the KMS service)
#   - The KMS, gateway, and dashboard services already running

param(
    [string]$TenantSlug = "maat-demo",
    [string]$TenantName = "Maat Demo",
    [string]$AdminEmail = "admin@maat-demo.local",
    [string]$AdminPassword = "demo-password",
    [string]$ApiKeyName = "demo-key",
    [string]$DemoStoreDb = "postgres://localhost/maat_demo_store",
    [string]$DemoStoreUrl = "http://127.0.0.1:8090",
    [string]$CustomerEmail = "demo-shopper@maat-demo.local"
)

$ErrorActionPreference = "Stop"

if (-not $env:STRIPE_SECRET_KEY) {
    Write-Error "STRIPE_SECRET_KEY env var must be set (use a sk_test_... key)"
    exit 1
}
if (-not ($env:STRIPE_SECRET_KEY.StartsWith("sk_test_"))) {
    Write-Warning "STRIPE_SECRET_KEY is not a test key. Proceed only if you mean to charge real cards."
}
if ((-not $env:DATABASE_URL) -and (-not $env:MAAT_CONFIG_DATABASE_URL)) {
    Write-Error "Set DATABASE_URL (or MAAT_CONFIG_DATABASE_URL) to the gateway DB before running setup."
    exit 1
}
if (-not $env:MAAT_KMS_AUTH_TOKEN) {
    Write-Error "MAAT_KMS_AUTH_TOKEN env var must be set (the same value the KMS was started with). Generate one with: cargo run -p maat-kms --bin maat-kmsd -- generate-auth-token"
    exit 1
}

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$workspace = Split-Path -Parent $here

function SqlEscape {
    param([string]$s)
    if ($null -eq $s) { return "" }
    return $s.Replace("'", "''")
}

function RunSql {
    param([string]$sql)
    $sql | psql -U postgres -W $DemoStoreDb -v ON_ERROR_STOP=1 -q | Out-Null
}

function RunSqlScalar {
    param([string]$sql)
    $out = $sql | psql -U postgres -W $DemoStoreDb -v ON_ERROR_STOP=1 -t -A
    return $out.Trim()
}

# Run the bootstrap binary and capture stdout. Bootstrap currently emits
# human-readable text, with progress lines on stderr. PowerShell's default
# ErrorActionPreference treats native stderr as a terminating error, so we
# redirect stderr->stdout and then check $LASTEXITCODE explicitly.
function Run-Bootstrap {
    param([string[]]$Arguments)
    # Save and relax ErrorActionPreference around the native call so that
    # progress text on stderr (e.g. "Generating executor key in KMS...") is
    # captured rather than treated as a script-fatal error.
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        $output = & cargo run -q -p maat-gateway --bin maat-gateway-bootstrap -- @Arguments 2>&1 | ForEach-Object { $_.ToString() }
    } finally {
        $ErrorActionPreference = $prev
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host "----- bootstrap output -----"
        $output | ForEach-Object { Write-Host $_ }
        Write-Host "----------------------------"
        Write-Error "maat-gateway-bootstrap failed (exit $LASTEXITCODE)"
        exit 1
    }
    return $output
}

function Extract-Field {
    param([string[]]$Output, [string]$Label)
    foreach ($line in $Output) {
        if ($line -match "^\s*$([regex]::Escape($Label))\s*:?\s+(.+)$") {
            return $matches[1].Trim()
        }
    }
    return $null
}

Write-Host "Provisioning Maat tenant..."
Push-Location $workspace
try {
    $tenantOut = Run-Bootstrap @("create-tenant", $TenantSlug, $TenantName)
    $tenantId = Extract-Field -Output $tenantOut -Label "ID"
    if (-not $tenantId) {
        # Maybe the tenant already exists; try to look it up by re-running
        # api-key creation — tenants persist across re-runs.
        Write-Host "  (tenant may already exist; continuing)"
    } else {
        Write-Host ("  tenant_id = {0}" -f $tenantId)
    }

    Write-Host "Provisioning API key..."
    $keyOut = Run-Bootstrap @("create-api-key", $TenantSlug, $ApiKeyName)

    # The full key prints on its own line, indented two spaces, between
    # blank lines. Format is `mgw_live_<27-char base64url>`. Base64url
    # uses [A-Za-z0-9_-], so the regex must include `-`.
    $apiKey = $null
    foreach ($line in $keyOut) {
        $trimmed = $line.ToString().Trim()
        if ($trimmed -match "^mgw_live_[A-Za-z0-9_\-]{20,}$") {
            $apiKey = $trimmed
            break
        }
    }
    if (-not $apiKey) {
        Write-Host "----- bootstrap output (could not find key) -----"
        $keyOut | ForEach-Object { Write-Host "    $_" }
        Write-Host "-------------------------------------------------"
        Write-Error "Could not extract API key from bootstrap output. Run create-api-key manually and pass the key to subsequent commands."
        exit 1
    }
    # Defensive: confirm we captured the entire key by comparing the line
    # we're about to use to the surrounding context — print the captured
    # key so the operator can visually confirm it matches the bootstrap
    # output above.
    Write-Host ("  api_key   = {0}" -f $apiKey)

    Write-Host "Provisioning admin user..."
    $userOut = Run-Bootstrap @("create-user", $TenantSlug, $AdminEmail, $AdminPassword, "admin")
    Write-Host ("  user      = {0}" -f $AdminEmail)
} finally {
    Pop-Location
}

Write-Host ""
Write-Host "Creating Stripe test customer with attached test card..."
$stripeBase = "https://api.stripe.com/v1"

$custJson = curl.exe -s -u "$($env:STRIPE_SECRET_KEY):" "$stripeBase/customers" `
    -d "email=$CustomerEmail" `
    -d "name=Demo Shopper"
$cust = $custJson | ConvertFrom-Json
if (-not $cust.id) {
    Write-Error ("Stripe customer creation failed: {0}" -f $custJson)
    exit 1
}
$stripeCustomerId = $cust.id
Write-Host ("  stripe customer = {0}" -f $stripeCustomerId)

$pmJson = curl.exe -s -u "$($env:STRIPE_SECRET_KEY):" "$stripeBase/payment_methods" `
    -d "type=card" `
    -d "card[token]=tok_visa"
$pm = $pmJson | ConvertFrom-Json
if (-not $pm.id) {
    Write-Error ("Stripe payment method creation failed: {0}" -f $pmJson)
    exit 1
}
$paymentMethodId = $pm.id
Write-Host ("  payment method  = {0}" -f $paymentMethodId)

curl.exe -s -u "$($env:STRIPE_SECRET_KEY):" "$stripeBase/payment_methods/$paymentMethodId/attach" `
    -d "customer=$stripeCustomerId" | Out-Null
curl.exe -s -u "$($env:STRIPE_SECRET_KEY):" -X POST "$stripeBase/customers/$stripeCustomerId" `
    -d "invoice_settings[default_payment_method]=$paymentMethodId" | Out-Null

Write-Host ""
Write-Host "Seeding demo store with products..."
$seedFile = Join-Path $here "seed-products.json"
$products = Get-Content $seedFile -Raw | ConvertFrom-Json

foreach ($p in $products) {
    $sku = SqlEscape $p.sku
    $name = SqlEscape $p.name
    $desc = SqlEscape $p.description
    $price = $p.price_cents
    $currency = SqlEscape $p.currency
    $imgClause = "NULL"
    if ($p.image_url) {
        $img = SqlEscape $p.image_url
        $imgClause = "'$img'"
    }
    $sql = "INSERT INTO products (sku, name, description, price_cents, currency, image_url) " +
           "VALUES ('$sku', '$name', '$desc', $price, '$currency', $imgClause) " +
           "ON CONFLICT (sku) DO UPDATE SET " +
           "name = EXCLUDED.name, description = EXCLUDED.description, " +
           "price_cents = EXCLUDED.price_cents, currency = EXCLUDED.currency, " +
           "image_url = EXCLUDED.image_url;"
    RunSql $sql
}
Write-Host ("  {0} products seeded" -f $products.Count)

$emailEsc = SqlEscape $CustomerEmail
$cidEsc = SqlEscape $stripeCustomerId
$customerSql = "INSERT INTO customers (email, stripe_customer_id) " +
               "VALUES ('$emailEsc', '$cidEsc') " +
               "ON CONFLICT (email) DO UPDATE SET stripe_customer_id = EXCLUDED.stripe_customer_id " +
               "RETURNING id;"
$demoCustomerId = RunSqlScalar $customerSql
Write-Host ("  demo customer id = {0}" -f $demoCustomerId)

Write-Host ""
Write-Host "Done."
Write-Host ""
Write-Host "Save these for the demo:"
Write-Host ("  TENANT_SLUG            = {0}" -f $TenantSlug)
Write-Host ("  ADMIN_EMAIL            = {0}" -f $AdminEmail)
Write-Host ("  ADMIN_PASSWORD         = {0}" -f $AdminPassword)
Write-Host ("  MAAT_API_KEY           = {0}" -f $apiKey)
Write-Host ("  STRIPE_CUSTOMER_ID     = {0}" -f $stripeCustomerId)
Write-Host ("  DEMO_AGENT_CUSTOMER_ID = {0}" -f $demoCustomerId)
Write-Host ""
Write-Host "Environment for the store binary:"
Write-Host ('  $env:DEMO_STORE_DATABASE_URL = "' + $DemoStoreDb + '"')
Write-Host '  $env:STRIPE_SECRET_KEY       = "<your sk_test_... key>"'
Write-Host '  $env:MAAT_GATEWAY_URL        = "http://127.0.0.1:8080"'
Write-Host ('  $env:MAAT_API_KEY            = "' + $apiKey + '"')
Write-Host ""
Write-Host "Environment for the agent binary:"
Write-Host ('  $env:MAAT_API_KEY              = "' + $apiKey + '"')
Write-Host ('  $env:DEMO_AGENT_CUSTOMER_ID    = "' + $demoCustomerId + '"')
Write-Host '  $env:DEMO_AGENT_GATEWAY_URL    = "http://127.0.0.1:8080"'
Write-Host ('  $env:DEMO_AGENT_STORE_URL      = "' + $DemoStoreUrl + '"')
Write-Host ""
Write-Host "Reminder: the KMS, gateway, and dashboard ALL need MAAT_KMS_AUTH_TOKEN"
Write-Host "set to the same value. The store and agent do not - they only talk to the"
Write-Host "gateway/store, never the KMS directly."
Write-Host ""
Write-Host "Now read demo/README.md for the 10-step demo flow."
