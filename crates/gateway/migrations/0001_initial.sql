-- Maat Gateway — initial schema (Slice 2)
--
-- Creates the receipts table, its indexes, and an append-only trigger that
-- prevents any UPDATE or DELETE against it. Receipts are immutable evidence;
-- tampering with them defeats the purpose of the system.
--
-- Apply with:
--   sqlx migrate run
-- Or manually:
--   psql "$DATABASE_URL" -f migrations/0001_initial.sql

CREATE TABLE receipts (
    -- The receipt's ObjectId (SHA-256 over the canonical signing payload).
    -- 32 bytes, primary key, content-addressed.
    id                    BYTEA PRIMARY KEY,

    -- Gateway executor public key that signed this receipt.
    -- Kept for future multi-tenancy and key rotation (receipts signed by an
    -- older key remain verifiable against that key).
    executor_pubkey       BYTEA NOT NULL,

    -- Leaf delegation's ObjectId. The receipt attests to verification of
    -- this specific delegation (and its chain).
    delegation_id         BYTEA NOT NULL,

    -- Anchor's ObjectId. Links the receipt to the state the agent committed to.
    anchor_id             BYTEA NOT NULL,

    -- Verification outcome: 'Success', 'Failure', or 'Partial'.
    -- Stored as text for readability; enforced by a CHECK constraint.
    outcome               TEXT NOT NULL
                          CHECK (outcome IN ('Success', 'Failure', 'Partial')),

    -- The action scope atom that was requested (e.g., "finance:payment:execute").
    action_scope          TEXT NOT NULL,

    -- When the gateway executed the verification (from the receipt).
    executed_at           TIMESTAMPTZ NOT NULL,

    -- When this row was persisted by the gateway. Separate from executed_at
    -- so we can detect clock skew and out-of-order writes in audits.
    stored_at             TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- The full delegation chain, root-to-leaf, as an array of ObjectIds.
    -- We keep this denormalized rather than normalizing into a join table
    -- because chains are small (typically 1-3 entries) and we almost always
    -- want the full chain when we have a receipt.
    delegation_chain      BYTEA[] NOT NULL,

    -- The leaf delegation's agent public key. Extracted from the delegation
    -- at write time so we can index on it without parsing the JSON on read.
    agent_pubkey          BYTEA NOT NULL,

    -- The full receipt object as JSON. Source of truth for everything above.
    -- In the event of schema questions, this is the authoritative record.
    receipt_json          JSONB NOT NULL
);

-- ─── Indexes ───
-- These match the dashboard's primary query patterns from the architecture doc:
--   "receipts for agent X in time range Y"
--   "receipts by outcome (how many rejections this hour?)"
--   "receipts for delegation Z (audit a specific chain)"
--   "receipts for scope S (what's this agent doing with finance:payment?)"

CREATE INDEX receipts_stored_at_idx     ON receipts (stored_at DESC);
CREATE INDEX receipts_agent_time_idx    ON receipts (agent_pubkey, stored_at DESC);
CREATE INDEX receipts_delegation_idx    ON receipts (delegation_id);
CREATE INDEX receipts_scope_time_idx    ON receipts (action_scope, stored_at DESC);
CREATE INDEX receipts_outcome_time_idx  ON receipts (outcome, stored_at DESC);

-- ─── Append-only enforcement ───
-- Receipts are evidence. They must never be modified or deleted.
-- This trigger enforces that at the database level so no application bug,
-- no rogue migration, and no hand-typed UPDATE can tamper with history.
--
-- Administrative deletes for retention policy go through a separate
-- privileged path (not yet implemented — Slice 9 territory).

CREATE OR REPLACE FUNCTION receipts_reject_mutation() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'receipts are append-only: %s forbidden', TG_OP;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER receipts_no_update
    BEFORE UPDATE ON receipts
    FOR EACH ROW EXECUTE FUNCTION receipts_reject_mutation();

CREATE TRIGGER receipts_no_delete
    BEFORE DELETE ON receipts
    FOR EACH ROW EXECUTE FUNCTION receipts_reject_mutation();
