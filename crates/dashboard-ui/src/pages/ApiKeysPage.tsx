import { useState, type FormEvent } from "react";
import { Page } from "../components/Page";
import { DataTable, type Column } from "../components/DataTable";
import { CopyText } from "../components/CopyText";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { tenantApi } from "../api/client";
import type { ApiKey, ApiKeyCreated } from "../api/types";
import { formatRelative } from "../lib/format";

export function ApiKeysPage() {
  const { state } = useAuth();
  const isAdmin =
    state.status === "authenticated" && state.user.role === "admin";

  const fetchState = useFetch(() => tenantApi.listApiKeys());
  const [creating, setCreating] = useState(false);
  const [revealed, setRevealed] = useState<ApiKeyCreated | null>(null);

  const columns: Column<ApiKey>[] = [
    {
      key: "name",
      header: "Name",
      render: (k) => <span className="text-ink-100">{k.name}</span>,
    },
    {
      key: "prefix",
      header: "Key",
      render: (k) => (
        <span className="font-mono text-sm text-ink-300">
          mgw_live_{k.key_prefix}
          <span className="text-ink-600">…</span>
        </span>
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
      key: "last_used",
      header: "Last used",
      align: "right",
      render: (k) => (
        <span
          className={k.last_used_at ? "text-ink-300" : "text-ink-600 italic"}
          title={k.last_used_at ?? ""}
        >
          {k.last_used_at ? formatRelative(k.last_used_at) : "never"}
        </span>
      ),
    },
    {
      key: "status",
      header: "Status",
      render: (k) =>
        k.revoked_at ? (
          <span className="pill-bad">revoked</span>
        ) : (
          <span className="pill-ok">active</span>
        ),
    },
  ];

  const action = isAdmin && (
    <button onClick={() => setCreating(true)} className="btn-primary">
      Create API key
    </button>
  );

  return (
    <Page
      title="API keys"
      description="Bearer credentials used by agents and integrations to call the verification service."
      action={action}
    >
      {fetchState.loading && <Loading />}
      {fetchState.error && (
        <ErrorState error={fetchState.error} onRetry={fetchState.reload} />
      )}
      {fetchState.data && fetchState.data.length === 0 && (
        <EmptyState
          title="No API keys yet"
          hint="Create one to authenticate calls to the verification service."
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
                  !k.revoked_at && (
                    <button
                      onClick={async () => {
                        if (
                          !confirm(
                            `Revoke API key "${k.name}"?\n\nAny agent or integration using this key will start receiving 401.`,
                          )
                        )
                          return;
                        await tenantApi.revokeApiKey(k.id);
                        fetchState.reload();
                      }}
                      className="btn-danger text-xs"
                    >
                      Revoke
                    </button>
                  )
              : undefined
          }
        />
      )}

      <Modal
        open={creating}
        onClose={() => setCreating(false)}
        title="Create API key"
      >
        <CreateForm
          onCancel={() => setCreating(false)}
          onCreated={(created) => {
            setCreating(false);
            setRevealed(created);
            fetchState.reload();
          }}
        />
      </Modal>

      <Modal
        open={!!revealed}
        onClose={() => setRevealed(null)}
        title="API key created"
        blocking
      >
        {revealed && (
          <RevealForm
            created={revealed}
            onAcknowledge={() => setRevealed(null)}
          />
        )}
      </Modal>
    </Page>
  );
}

function CreateForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: (created: ApiKeyCreated) => void;
}) {
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const created = await tenantApi.createApiKey(name.trim());
      onCreated(created);
    } catch (err) {
      setError(err instanceof Error ? err.message : "create failed");
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <form onSubmit={onSubmit} className="space-y-4">
      <div>
        <label htmlFor="ak-name" className="label">
          Name
        </label>
        <input
          id="ak-name"
          className="input"
          value={name}
          onChange={(e) => setName(e.target.value)}
          required
          autoFocus
          placeholder="production-agent"
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
          {submitting ? "Creating…" : "Create"}
        </button>
      </div>
    </form>
  );
}

function RevealForm({
  created,
  onAcknowledge,
}: {
  created: ApiKeyCreated;
  onAcknowledge: () => void;
}) {
  const [acknowledged, setAcknowledged] = useState(false);

  return (
    <div className="space-y-5">
      <div className="bg-warn-500/10 border border-warn-500/30 rounded-md p-3">
        <p className="text-sm text-warn-400 font-medium">
          This is the only time the full key will be shown.
        </p>
        <p className="text-xs text-ink-300 mt-1">
          If you lose it, you'll need to revoke this key and create a new
          one.
        </p>
      </div>

      <div>
        <div className="label">{created.metadata.name}</div>
        <div className="bg-ink-950 border border-ink-700 rounded-md p-3">
          <CopyText
            value={created.full_key}
            display={created.full_key}
            className="break-all w-full text-xs"
          />
        </div>
      </div>

      <label className="flex items-start gap-2 text-sm cursor-pointer">
        <input
          type="checkbox"
          checked={acknowledged}
          onChange={(e) => setAcknowledged(e.target.checked)}
          className="mt-0.5 h-4 w-4 rounded border-ink-600 bg-ink-950
                     text-accent-500 focus:ring-accent-500 focus:ring-offset-0"
        />
        <span className="text-ink-300">
          I've copied this key somewhere safe.
        </span>
      </label>

      <div className="flex justify-end">
        <button
          onClick={onAcknowledge}
          disabled={!acknowledged}
          className="btn-primary"
        >
          Done
        </button>
      </div>
    </div>
  );
}
