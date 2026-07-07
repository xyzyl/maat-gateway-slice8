// Ledger panel — shown in the delegation detail modal.
//
// Reads /dashboard/v1/delegations/:id/ledger and renders the cumulative
// state. Polls on demand (no auto-refresh — operators reopen the modal
// when they want a fresh view).

import { useFetch } from "../lib/useFetch";
import { delegationsApi } from "../api/client";
import { formatMoney } from "./ConstraintBuilder";
import { formatRelative } from "../lib/format";
import { Loading } from "./States";

export function LedgerPanel({ delegationIdB64 }: { delegationIdB64: string }) {
  const { data, error, loading } = useFetch(
    () => delegationsApi.ledger(delegationIdB64),
    [delegationIdB64],
  );

  if (loading) return <Loading label="Loading ledger" />;
  if (error) {
    return (
      <p className="text-xs text-bad-400">
        Could not load ledger: {error.message}
      </p>
    );
  }
  if (!data) return null;

  if (!data.cap && data.recent_entries.length === 0) {
    return (
      <p className="text-xs text-ink-500 italic">
        No cumulative cap set; no ledger activity yet.
      </p>
    );
  }

  const used = data.recorded_total;
  const remaining = data.cap ? Math.max(0, data.cap.amount - used) : null;
  const pct = data.cap ? Math.min(100, (used / data.cap.amount) * 100) : 0;

  return (
    <div className="space-y-3">
      {data.cap && (
        <div>
          <div className="flex items-baseline justify-between">
            <span className="text-xs uppercase tracking-wider text-ink-400">
              Cumulative spend
            </span>
            <span className="font-mono text-sm tabular-nums text-ink-200">
              {formatMoney(used, data.cap.decimals)} /{" "}
              {formatMoney(data.cap.amount, data.cap.decimals)}{" "}
              {data.cap.currency}
            </span>
          </div>
          <div className="mt-1.5 h-1.5 bg-ink-800 rounded-full overflow-hidden">
            <div
              className={`h-full ${
                pct >= 90
                  ? "bg-bad-500"
                  : pct >= 70
                    ? "bg-warn-500"
                    : "bg-accent-500"
              }`}
              style={{ width: `${pct}%` }}
            />
          </div>
          {remaining !== null && (
            <p className="text-xs text-ink-500 mt-1">
              {formatMoney(remaining, data.cap.decimals)} {data.cap.currency}{" "}
              remaining
            </p>
          )}
        </div>
      )}

      {data.recent_entries.length > 0 && (
        <div>
          <div className="text-xs uppercase tracking-wider text-ink-400 mb-1.5">
            Recent activity
          </div>
          <div className="space-y-1">
            {data.recent_entries.map((e) => (
              <div
                key={e.receipt_id_b64}
                className="flex items-center justify-between text-xs font-mono"
              >
                <span
                  className={
                    e.reversed_at ? "text-ink-600 line-through" : "text-ink-300"
                  }
                >
                  {e.amount !== null && e.decimals !== null
                    ? `${formatMoney(e.amount, e.decimals)} ${e.currency ?? ""}`
                    : "—"}
                </span>
                <span className="text-ink-500" title={e.recorded_at}>
                  {formatRelative(e.recorded_at)}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
