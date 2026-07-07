import { useMemo, useState, type FormEvent } from "react";
import { Page } from "../components/Page";
import { DataTable, type Column } from "../components/DataTable";
import { CopyText } from "../components/CopyText";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import {
  ConstraintBuilder,
  type ConstraintBuilderState,
} from "../components/ConstraintBuilder";
import { LedgerPanel } from "../components/LedgerPanel";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { delegationsApi, principalKeysApi } from "../api/client";
import type { DelegationView } from "../api/types";
import { formatRelative, shortenId, unixSeconds } from "../lib/format";

export function DelegationsPage() {
  const { state } = useAuth();
  const isAdmin =
    state.status === "authenticated" && state.user.role === "admin";

  const fetchState = useFetch(() => delegationsApi.list(200, 0));
  const [creating, setCreating] = useState(false);
  const [viewing, setViewing] = useState<DelegationView | null>(null);

  const columns: Column<DelegationView>[] = [
    {
      key: "id",
      header: "Delegation ID",
      render: (d) => (
        <CopyText value={d.id_b64} display={shortenId(d.id_b64, 8, 6)} />
      ),
    },
    {
      key: "created",
      header: "Created",
      align: "right",
      render: (d) => (
        <span className="text-ink-400" title={d.created_at}>
          {formatRelative(d.created_at)}
        </span>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: (d) =>
        d.revoked_at ? (
          <span
            className="pill-bad"
            title={d.revocation_reason ?? "revoked"}
          >
            revoked
          </span>
        ) : (
          <span className="pill-ok">active</span>
        ),
    },
  ];

  const action = isAdmin && (
    <button onClick={() => setCreating(true)} className="btn-primary">
      Create delegation
    </button>
  );

  return (
    <Page
      title="Delegations"
      description="Signed authorizations granting agents the right to act within a specific scope."
      action={action}
    >
      {fetchState.loading && <Loading />}
      {fetchState.error && (
        <ErrorState error={fetchState.error} onRetry={fetchState.reload} />
      )}
      {fetchState.data?.delegations.length === 0 && (
        <EmptyState
          title="No delegations yet"
          hint="Create one to authorize an agent to act on behalf of your tenant."
          action={action}
        />
      )}
      {fetchState.data && fetchState.data.delegations.length > 0 && (
        <DataTable
          columns={columns}
          rows={fetchState.data.delegations}
          rowKey={(d) => d.id_b64}
          rowAction={(d) => (
            <div className="flex justify-end gap-2">
              <button
                onClick={() => setViewing(d)}
                className="btn-ghost text-xs"
              >
                Inspect
              </button>
              {isAdmin && !d.revoked_at && (
                <button
                  onClick={async () => {
                    const reason = prompt("Reason for revocation? (optional)") ?? "";
                    if (
                      !confirm(
                        `Revoke delegation ${shortenId(d.id_b64, 6, 4)}?\n\nThis is permanent.`,
                      )
                    )
                      return;
                    try {
                      await delegationsApi.revoke(
                        d.id_b64,
                        reason.trim() || undefined,
                      );
                      fetchState.reload();
                    } catch (err) {
                      alert(
                        err instanceof Error ? err.message : "revoke failed",
                      );
                    }
                  }}
                  className="btn-danger text-xs"
                >
                  Revoke
                </button>
              )}
            </div>
          )}
        />
      )}

      <Modal
        open={creating}
        onClose={() => setCreating(false)}
        title="Create delegation"
        wide
      >
        <CreateDelegationForm
          onCancel={() => setCreating(false)}
          onCreated={() => {
            setCreating(false);
            fetchState.reload();
          }}
        />
      </Modal>

      <Modal
        open={!!viewing}
        onClose={() => setViewing(null)}
        title="Delegation details"
        wide
      >
        {viewing && <DelegationDetail d={viewing} />}
      </Modal>
    </Page>
  );
}

function DelegationDetail({ d }: { d: DelegationView }) {
  return (
    <div className="space-y-4">
      <div>
        <div className="label">ID</div>
        <CopyText value={d.id_b64} />
      </div>
      <div>
        <div className="label">Created</div>
        <p className="text-sm text-ink-200">
          {new Date(d.created_at).toLocaleString()}
        </p>
      </div>
      {d.revoked_at && (
        <div>
          <div className="label">Revoked</div>
          <p className="text-sm text-bad-400">
            {new Date(d.revoked_at).toLocaleString()}
            {d.revocation_reason && (
              <span className="text-ink-400">
                {" "}
                — {d.revocation_reason}
              </span>
            )}
          </p>
        </div>
      )}
      <div>
        <div className="label">Ledger</div>
        <LedgerPanel delegationIdB64={d.id_b64} />
      </div>
      <div>
        <div className="label">Signed delegation (JSON)</div>
        <pre
          className="text-xs font-mono text-ink-300 bg-ink-950 border border-ink-800
                     rounded-md p-3 max-h-64 overflow-auto"
        >
          {JSON.stringify(d.delegation, null, 2)}
        </pre>
      </div>
    </div>
  );
}

function CreateDelegationForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: () => void;
}) {
  const principalKeys = useFetch(() => principalKeysApi.list());

  const [principalKeyId, setPrincipalKeyId] = useState("");
  const [agentPubkey, setAgentPubkey] = useState("");
  const [scopes, setScopes] = useState("");
  // Default expiry: 1 hour from now. Operators almost always tighten this
  // up, but a short default is the safer pre-fill than something long.
  const [hoursFromNow, setHoursFromNow] = useState(1);
  const [constraintState, setConstraintState] = useState<ConstraintBuilderState>(
    { constraints: [], cumulativeCap: null },
  );
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeKeys = useMemo(
    () => principalKeys.data?.filter((k) => !k.retired_at) ?? [],
    [principalKeys.data],
  );

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const scopeArray = scopes
        .split(/[\s,]+/)
        .map((s) => s.trim())
        .filter(Boolean);
      if (scopeArray.length === 0) {
        throw new Error("At least one scope grant is required.");
      }

      const now = unixSeconds(new Date());
      const notAfter = now + Math.floor(hoursFromNow * 3600);

      await delegationsApi.create({
        principal_key_id: principalKeyId,
        agent_pubkey_b64: agentPubkey.trim(),
        scope_grants: scopeArray,
        not_before: now,
        not_after: notAfter,
        constraints:
          constraintState.constraints.length > 0
            ? constraintState.constraints
            : undefined,
        cumulative_cap: constraintState.cumulativeCap ?? undefined,
      });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "create failed");
    } finally {
      setSubmitting(false);
    }
  };

  if (principalKeys.loading) return <Loading />;
  if (activeKeys.length === 0) {
    return (
      <div className="space-y-4">
        <p className="text-sm text-ink-300">
          You need at least one active principal key to create a delegation.
        </p>
        <div className="flex justify-end">
          <button onClick={onCancel} className="btn-ghost">
            Close
          </button>
        </div>
      </div>
    );
  }

  return (
    <form onSubmit={onSubmit} className="space-y-4">
      <div>
        <label htmlFor="d-pk" className="label">
          Principal key
        </label>
        <select
          id="d-pk"
          className="input"
          value={principalKeyId}
          onChange={(e) => setPrincipalKeyId(e.target.value)}
          required
        >
          <option value="">Select…</option>
          {activeKeys.map((k) => (
            <option key={k.id} value={k.id}>
              {k.name}
            </option>
          ))}
        </select>
      </div>

      <div>
        <label htmlFor="d-agent" className="label">
          Agent public key (base64url, 32 bytes)
        </label>
        <textarea
          id="d-agent"
          className="input-mono h-20 resize-none break-all"
          required
          value={agentPubkey}
          onChange={(e) => setAgentPubkey(e.target.value)}
          placeholder="paste agent public key here"
        />
      </div>

      <div>
        <label htmlFor="d-scopes" className="label">
          Scope grants
        </label>
        <input
          id="d-scopes"
          className="input-mono"
          required
          value={scopes}
          onChange={(e) => setScopes(e.target.value)}
          placeholder="finance:payment:execute, ops:read"
        />
        <p className="text-xs text-ink-500 mt-1.5">
          Comma- or space-separated. Example:{" "}
          <code className="text-ink-300">test:action</code>
        </p>
      </div>

      <div>
        <label htmlFor="d-expiry" className="label">
          Expires in (hours)
        </label>
        <input
          id="d-expiry"
          type="number"
          min="0.1"
          step="0.1"
          className="input"
          value={hoursFromNow}
          onChange={(e) => setHoursFromNow(parseFloat(e.target.value))}
        />
      </div>

      <div className="border-t border-ink-800 pt-4">
        <ConstraintBuilder
          value={constraintState}
          onChange={setConstraintState}
        />
      </div>

      {error && (
        <div className="text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2">
          {error}
        </div>
      )}

      <div className="flex justify-end gap-2 pt-2">
        <button
          type="button"
          onClick={onCancel}
          className="btn-ghost"
          disabled={submitting}
        >
          Cancel
        </button>
        <button type="submit" className="btn-primary" disabled={submitting}>
          {submitting ? "Signing…" : "Create & sign"}
        </button>
      </div>
    </form>
  );
}
