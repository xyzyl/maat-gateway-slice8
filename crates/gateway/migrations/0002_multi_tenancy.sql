-- Maat Gateway — multi-tenancy schema (Slice 4)
--
-- Adds:
--   - tenants, users, api_keys, sessions, principal_keys, delegations tables
--   - tenant_id column on the existing receipts table
--   - tenant-leading composite indexes on receipts
--
-- Apply with:
--   psql $DATABASE_URL -f migrations/0002_multi_tenancy.sql

-- ─── tenants ───
CREATE TABLE tenants (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug                     TEXT NOT NULL UNIQUE,
    name                     TEXT NOT NULL,
    kms_executor_key_id      TEXT NOT NULL,
    settings_json            JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX tenants_slug_idx ON tenants (slug);

-- ─── users ───
CREATE TABLE users (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    email                    TEXT NOT NULL,
    password_hash            TEXT NOT NULL,
    role                     TEXT NOT NULL CHECK (role IN ('admin', 'viewer')),
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_login_at            TIMESTAMPTZ,
    UNIQUE (tenant_id, email)
);

CREATE INDEX users_tenant_idx ON users (tenant_id);

-- ─── api_keys ───
CREATE TABLE api_keys (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name                     TEXT NOT NULL,
    key_prefix               TEXT NOT NULL,
    key_hash                 TEXT NOT NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at             TIMESTAMPTZ,
    revoked_at               TIMESTAMPTZ
);

CREATE INDEX api_keys_tenant_idx ON api_keys (tenant_id);
CREATE INDEX api_keys_prefix_idx ON api_keys (key_prefix);

-- ─── sessions ───
CREATE TABLE sessions (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id                  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at               TIMESTAMPTZ NOT NULL,
    last_seen_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX sessions_user_idx ON sessions (user_id);
CREATE INDEX sessions_expires_idx ON sessions (expires_at);

-- ─── principal_keys ───
CREATE TABLE principal_keys (
    id                       UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id                UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    name                     TEXT NOT NULL,
    kms_key_id               TEXT NOT NULL,
    public_key_b64           TEXT NOT NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    retired_at               TIMESTAMPTZ
);

CREATE INDEX principal_keys_tenant_idx ON principal_keys (tenant_id);

-- ─── delegations ───
CREATE TABLE delegations (
    id                       BYTEA PRIMARY KEY,
    tenant_id                UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    principal_pubkey         BYTEA NOT NULL,
    agent_pubkey             BYTEA NOT NULL,
    parent_id                BYTEA,
    not_before               TIMESTAMPTZ NOT NULL,
    not_after                TIMESTAMPTZ NOT NULL,
    scope_grants             TEXT[] NOT NULL,
    delegation_json          JSONB NOT NULL,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at               TIMESTAMPTZ,
    revocation_reason        TEXT
);

CREATE INDEX delegations_tenant_created_idx ON delegations (tenant_id, created_at DESC);
CREATE INDEX delegations_tenant_agent_idx   ON delegations (tenant_id, agent_pubkey);

-- Delegations are append-only on core fields; only revoked_at and
-- revocation_reason may be updated.
CREATE OR REPLACE FUNCTION delegations_reject_core_mutations() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'delegations are append-only: DELETE forbidden';
    END IF;
    IF OLD.id             IS DISTINCT FROM NEW.id             OR
       OLD.tenant_id      IS DISTINCT FROM NEW.tenant_id      OR
       OLD.principal_pubkey IS DISTINCT FROM NEW.principal_pubkey OR
       OLD.agent_pubkey   IS DISTINCT FROM NEW.agent_pubkey   OR
       OLD.delegation_json IS DISTINCT FROM NEW.delegation_json OR
       OLD.created_at     IS DISTINCT FROM NEW.created_at
    THEN
        RAISE EXCEPTION 'delegation core fields are immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER delegations_no_core_mutation
    BEFORE UPDATE OR DELETE ON delegations
    FOR EACH ROW EXECUTE FUNCTION delegations_reject_core_mutations();

-- ─── receipts: add tenant_id ───
ALTER TABLE receipts ADD COLUMN tenant_id UUID;

DO $$
DECLARE
    legacy_tenant_id UUID;
BEGIN
    IF EXISTS (SELECT 1 FROM receipts WHERE tenant_id IS NULL LIMIT 1) THEN
        INSERT INTO tenants (slug, name, kms_executor_key_id)
        VALUES ('legacy', 'Legacy (pre-multi-tenancy)', 'unknown')
        ON CONFLICT (slug) DO NOTHING;

        SELECT id INTO legacy_tenant_id FROM tenants WHERE slug = 'legacy';
        UPDATE receipts SET tenant_id = legacy_tenant_id WHERE tenant_id IS NULL;
    END IF;
END $$;

ALTER TABLE receipts ALTER COLUMN tenant_id SET NOT NULL;
ALTER TABLE receipts ADD CONSTRAINT receipts_tenant_fk
    FOREIGN KEY (tenant_id) REFERENCES tenants(id);

DROP INDEX IF EXISTS receipts_stored_at_idx;
DROP INDEX IF EXISTS receipts_agent_time_idx;
DROP INDEX IF EXISTS receipts_delegation_idx;
DROP INDEX IF EXISTS receipts_scope_time_idx;
DROP INDEX IF EXISTS receipts_outcome_time_idx;

CREATE INDEX receipts_tenant_stored_idx       ON receipts (tenant_id, stored_at DESC);
CREATE INDEX receipts_tenant_agent_idx        ON receipts (tenant_id, agent_pubkey, stored_at DESC);
CREATE INDEX receipts_tenant_delegation_idx   ON receipts (tenant_id, delegation_id);
CREATE INDEX receipts_tenant_scope_idx        ON receipts (tenant_id, action_scope, stored_at DESC);
CREATE INDEX receipts_tenant_outcome_idx      ON receipts (tenant_id, outcome, stored_at DESC);
