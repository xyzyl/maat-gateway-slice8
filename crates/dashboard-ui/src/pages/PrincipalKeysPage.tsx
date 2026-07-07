import { useState, type FormEvent } from "react";
import { Page } from "../components/Page";
import { DataTable, type Column } from "../components/DataTable";
import { CopyText } from "../components/CopyText";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { principalKeysApi } from "../api/client";
import type { PrincipalKey } from "../api/types";
import { formatRelative, shortenId } from "../lib/format";

export function PrincipalKeysPage() {
  const { state } = useAuth();
  const isAdmin =
    state.status === "authenticated" && state.user.role === "admin";

  const fetchState = useFetch(() => principalKeysApi.list());
  const [creating, setCreating] = useState(false);

  const columns: Column<PrincipalKey>[] = [
    {
      key: "name",
      header: "Name",
      render: (k) => <span className="text-ink-100">{k.name}</span>,
    },
    {
      key: "kms_key_id",
      header: "KMS key id",
      render: (k) => (
        <CopyText
          value={k.kms_key_id}
          display={shortenId(k.kms_key_id, 8, 6)}
        />
      ),
    },
    {
      key: "public_key",
      header: "Public key",
      render: (k) => (
        <CopyText
          value={k.public_key_b64}
          display={shortenId(k.public_key_b64, 10, 6)}
        />
      ),
    },
    {
      key: "created",
      header: "Created",
      align: "right",
      render: (k) => (
        <span className="text-ink-400" title={k.created_at}>
          {formatRelative(k.created_at)}
        </span>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: (k) =>
        k.retired_at ? (
          <span className="pill-muted">retired</span>
        ) : (
          <span className="pill-ok">active</span>
        ),
    },
  ];

  const action = isAdmin && (
    <button onClick={() => setCreating(true)} className="btn-primary">
      Create principal key
    </button>
  );

  return (
    <Page
      title="Principal keys"
      description="Tenant signing keys used to authorize agents via delegations. Key material lives in the KMS."
      action={action}
    >
      {fetchState.loading && <Loading />}
      {fetchState.error && (
        <ErrorState error={fetchState.error} onRetry={fetchState.reload} />
      )}
      {fetchState.data && fetchState.data.length === 0 && (
        <EmptyState
          title="No principal keys yet"
          hint="Create one to start signing delegations."
          action={action}
        />
      )}
      {fetchState.data && fetchState.data.length > 0 && (
        <DataTable
          columns={columns}
          rows={fetchState.data}
          rowKey={(k) => k.id}
          rowAction={
            isAdmin
              ? (k) =>
                  !k.retired_at && (
                    <button
                      onClick={async () => {
                        if (!confirm(`Retire principal key "${k.name}"?`)) return;
                        await principalKeysApi.retire(k.id);
                        fetchState.reload();
                      }}
                      className="btn-ghost text-xs"
                    >
                      Retire
                    </button>
                  )
              : undefined
          }
        />
      )}

      <Modal
        open={creating}
        onClose={() => setCreating(false)}
        title="Create principal key"
      >
        <CreateForm
          onCancel={() => setCreating(false)}
          onCreated={() => {
            setCreating(false);
            fetchState.reload();
          }}
        />
      </Modal>
    </Page>
  );
}

function CreateForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await principalKeysApi.create(name.trim());
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "create failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={onSubmit} className="space-y-4">
      <div>
        <label htmlFor="pk-name" className="label">
          Name
        </label>
        <input
          id="pk-name"
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          autoFocus
          placeholder="primary"
        />
        <p className="text-xs text-ink-500 mt-1.5">
          Internal label only. The KMS generates the underlying key.
        </p>
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
          {submitting ? "Creating…" : "Create"}
        </button>
      </div>
    </form>
  );
}
