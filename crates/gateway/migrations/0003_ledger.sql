-- Slice 7: ledger entries + cumulative caps for delegations.
--
-- Ledger entries record every approved value-bearing verify request, keyed
-- by tenant + delegation. The Redis hot path is the source of truth for
-- atomic concurrency; this table is the durable record + query surface.
--
-- The cumulative cap lives alongside the delegation row. Optional —
-- delegations without a cumulative_cap_amount have no gateway-side
-- cumulative limit (only the protocol's per-action MaxValue applies).

CREATE TABLE ledger_entries (
    id              BIGSERIAL PRIMARY KEY,
    tenant_id       UUID NOT NULL,
    delegation_id   BYTEA NOT NULL,
    receipt_id      BYTEA NOT NULL,

    -- Recorded value claim (currency/amount/decimals from the verify request).
    -- NULL only for entries recording action-count enforcement (future use).
    currency        TEXT,
    amount          BIGINT,
    decimals        SMALLINT,

    recorded_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Reversal scaffolding (NULL today; populated by future Reversal flow).
    reversed_at     TIMESTAMPTZ,
    reversal_reason TEXT
);

CREATE INDEX idx_ledger_tenant_delegation
    ON ledger_entries (tenant_id, delegation_id, recorded_at);

-- Append-only at the trigger level (matches receipts).
CREATE OR REPLACE FUNCTION ledger_no_update_or_delete()
RETURNS TRIGGER AS $$
BEGIN
    -- Permit updates only to reversal columns.
    IF TG_OP = 'UPDATE' THEN
        IF (OLD.id, OLD.tenant_id, OLD.delegation_id, OLD.receipt_id,
            OLD.currency, OLD.amount, OLD.decimals, OLD.recorded_at)
           IS DISTINCT FROM
           (NEW.id, NEW.tenant_id, NEW.delegation_id, NEW.receipt_id,
            NEW.currency, NEW.amount, NEW.decimals, NEW.recorded_at)
        THEN
            RAISE EXCEPTION 'ledger_entries is append-only except for reversal columns';
        END IF;
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'ledger_entries is append-only';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ledger_immutable
    BEFORE UPDATE OR DELETE ON ledger_entries
    FOR EACH ROW EXECUTE FUNCTION ledger_no_update_or_delete();

-- Per-delegation cumulative cap. One row per delegation that has one.
-- Currency + amount + decimals match the protocol's MaxValue shape so
-- the units are unambiguous.
CREATE TABLE delegation_cumulative_caps (
    tenant_id     UUID NOT NULL,
    delegation_id BYTEA NOT NULL,
    currency      TEXT NOT NULL,
    amount        BIGINT NOT NULL,
    decimals      SMALLINT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (tenant_id, delegation_id)
);
