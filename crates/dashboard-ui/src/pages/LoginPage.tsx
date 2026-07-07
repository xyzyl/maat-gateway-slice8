import { useState, type FormEvent } from "react";
import { useAuth } from "../lib/auth";
import { ApiClientError } from "../api/client";

export function LoginPage() {
  const { login } = useAuth();
  const [tenant, setTenant] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const onSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await login(tenant.trim(), email.trim(), password);
    } catch (err) {
      if (err instanceof ApiClientError) {
        setError(err.message);
      } else {
        setError("Login failed. Check the dashboard service is reachable.");
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center px-4">
      {/* Subtle background grid for depth — helps the centered card not feel
          stranded on an empty void. */}
      <div
        aria-hidden
        className="fixed inset-0 pointer-events-none opacity-[0.06]
                   bg-[linear-gradient(to_right,#fff_1px,transparent_1px),
                       linear-gradient(to_bottom,#fff_1px,transparent_1px)]
                   bg-[size:32px_32px]"
      />

      <div className="relative w-full max-w-sm">
        <div className="mb-8 text-center">
          <div className="inline-flex items-center gap-2 mb-2">
            <span className="text-3xl font-semibold tracking-tight text-ink-50">
              maat
            </span>
            <span
              className="px-1.5 py-0.5 rounded text-[0.65rem] font-mono uppercase
                         tracking-wider text-accent-400 border border-accent-500/40"
            >
              dashboard
            </span>
          </div>
          <p className="text-sm text-ink-400">
            sign in to manage delegations and audit receipts
          </p>
        </div>

        <form onSubmit={onSubmit} className="card p-6 space-y-4">
          <div>
            <label htmlFor="tenant" className="label">
              Tenant
            </label>
            <input
              id="tenant"
              className="input-mono"
              autoComplete="organization"
              required
              autoFocus
              value={tenant}
              onChange={(e) => setTenant(e.target.value)}
              placeholder="acme"
            />
          </div>
          <div>
            <label htmlFor="email" className="label">
              Email
            </label>
            <input
              id="email"
              type="email"
              className="input"
              autoComplete="username"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
            />
          </div>
          <div>
            <label htmlFor="password" className="label">
              Password
            </label>
            <input
              id="password"
              type="password"
              className="input"
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
          </div>

          {error && (
            <div className="text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2">
              {error}
            </div>
          )}

          <button
            type="submit"
            disabled={submitting}
            className="btn-primary w-full"
          >
            {submitting ? "Signing in…" : "Sign in"}
          </button>
        </form>

        <p className="mt-6 text-center text-xs text-ink-500">
          New tenants are provisioned via{" "}
          <code className="font-mono text-ink-400">maat-gateway-bootstrap</code>
        </p>
      </div>
    </div>
  );
}
