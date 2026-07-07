import { useState, type FormEvent } from "react";
import { Page } from "../components/Page";
import { DataTable, type Column } from "../components/DataTable";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { usersApi } from "../api/client";
import type { UserListItem, UserRole } from "../api/types";
import { formatRelative } from "../lib/format";

export function UsersPage() {
  const { state } = useAuth();
  const isAdmin =
    state.status === "authenticated" && state.user.role === "admin";
  const myUserId =
    state.status === "authenticated" ? state.user.id : null;

  const fetchState = useFetch(() => usersApi.list());
  const [creating, setCreating] = useState(false);

  const columns: Column<UserListItem>[] = [
    {
      key: "email",
      header: "Email",
      render: (u) => (
        <span className="text-ink-100">
          {u.email}
          {u.id === myUserId && (
            <span className="ml-2 text-xs text-ink-500">(you)</span>
          )}
        </span>
      ),
    },
    {
      key: "role",
      header: "Role",
      render: (u) =>
        u.role === "admin" ? (
          <span className="pill-ok">admin</span>
        ) : (
          <span className="pill-muted">viewer</span>
        ),
    },
    {
      key: "created",
      header: "Created",
      align: "right",
      render: (u) => (
        <span className="text-ink-400" title={u.created_at}>
          {formatRelative(u.created_at)}
        </span>
      ),
    },
    {
      key: "last_login",
      header: "Last login",
      align: "right",
      render: (u) => (
        <span
          className={u.last_login_at ? "text-ink-300" : "text-ink-600 italic"}
          title={u.last_login_at ?? ""}
        >
          {u.last_login_at ? formatRelative(u.last_login_at) : "never"}
        </span>
      ),
    },
  ];

  const action = isAdmin && (
    <button onClick={() => setCreating(true)} className="btn-primary">
      Add user
    </button>
  );

  return (
    <Page
      title="Users"
      description="Dashboard accounts. Admins can manage all resources; viewers can only read."
      action={action}
    >
      {fetchState.loading && <Loading />}
      {fetchState.error && (
        <ErrorState error={fetchState.error} onRetry={fetchState.reload} />
      )}
      {fetchState.data && fetchState.data.length > 0 && (
        <DataTable
          columns={columns}
          rows={fetchState.data}
          rowKey={(u) => u.id}
          rowAction={
            isAdmin
              ? (u) =>
                  u.id !== myUserId && (
                    <button
                      onClick={async () => {
                        if (!confirm(`Remove user ${u.email}?`)) return;
                        try {
                          await usersApi.delete(u.id);
                          fetchState.reload();
                        } catch (err) {
                          alert(
                            err instanceof Error
                              ? err.message
                              : "delete failed",
                          );
                        }
                      }}
                      className="btn-danger text-xs"
                    >
                      Remove
                    </button>
                  )
              : undefined
          }
        />
      )}
      {fetchState.data && fetchState.data.length === 0 && (
        <EmptyState title="No users." />
      )}

      <Modal
        open={creating}
        onClose={() => setCreating(false)}
        title="Add user"
      >
        <CreateUserForm
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

function CreateUserForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: () => void;
}) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState<UserRole>("viewer");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await usersApi.create(email.trim(), password, role);
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
        <label htmlFor="u-email" className="label">
          Email
        </label>
        <input
          id="u-email"
          type="email"
          className="input"
          required
          autoFocus
          value={email}
          onChange={(e) => setEmail(e.target.value)}
        />
      </div>
      <div>
        <label htmlFor="u-pw" className="label">
          Initial password
        </label>
        <input
          id="u-pw"
          type="password"
          className="input"
          required
          minLength={8}
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        <p className="text-xs text-ink-500 mt-1.5">
          The user can change this after their first login.
        </p>
      </div>
      <div>
        <label htmlFor="u-role" className="label">
          Role
        </label>
        <select
          id="u-role"
          className="input"
          value={role}
          onChange={(e) => setRole(e.target.value as UserRole)}
        >
          <option value="viewer">viewer — read-only</option>
          <option value="admin">admin — full access</option>
        </select>
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
          {submitting ? "Adding…" : "Add"}
        </button>
      </div>
    </form>
  );
}
