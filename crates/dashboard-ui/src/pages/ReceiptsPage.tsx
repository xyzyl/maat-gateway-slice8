import { useState } from "react";
import { Page } from "../components/Page";
import { CopyText } from "../components/CopyText";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { receiptsApi } from "../api/client";
import { formatRelative, shortenId } from "../lib/format";

interface ReceiptShape {
  id?: string | number[];
  outcome?: string;
  action?: {
    scope_used?: string;
    description?: string;
    /** base64url-encoded bytes; for Slice 7, may contain a JSON-encoded ValueClaim. */
    value?: string;
  };
  delegation_id?: string | number[];
  executed_at?: number;
  detail?: string | null;
}

const PAGE_SIZE = 50;

export function ReceiptsPage() {
  const [page, setPage] = useState(0);
  const [scope, setScope] = useState("");
  const [outcome, setOutcome] = useState("");

  const fetchState = useFetch(
    () =>
      receiptsApi.list({
        scope: scope || undefined,
        outcome: outcome || undefined,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      }),
    [page, scope, outcome],
  );

  const filterBar = (
    <div className="flex flex-wrap gap-3 items-end">
      <div className="flex-1 min-w-[12rem]">
        <label htmlFor="r-scope" className="label">
          Scope filter
        </label>
        <input
          id="r-scope"
          className="input-mono"
          value={scope}
          onChange={(e) => {
            setScope(e.target.value);
            setPage(0);
          }}
          placeholder="any scope"
        />
      </div>
      <div>
        <label htmlFor="r-outcome" className="label">
          Outcome
        </label>
        <select
          id="r-outcome"
          className="input"
          value={outcome}
          onChange={(e) => {
            setOutcome(e.target.value);
            setPage(0);
          }}
        >
          <option value="">All</option>
          <option value="Success">Success</option>
          <option value="Failure">Failure</option>
          <option value="Partial">Partial</option>
        </select>
      </div>
    </div>
  );

  return (
    <Page
      title="Receipts"
      description="Cryptographic record of every verification — successes and rejections both."
    >
      <div className="card p-4">{filterBar}</div>

      {fetchState.loading && <Loading />}
      {fetchState.error && (
        <ErrorState error={fetchState.error} onRetry={fetchState.reload} />
      )}

      {fetchState.data && fetchState.data.receipts.length === 0 && (
        <EmptyState
          title={page === 0 ? "No receipts yet" : "No receipts on this page"}
          hint={
            page === 0
              ? "Receipts will appear here as agents call the verification service."
              : "Try lowering the page or relaxing your filters."
          }
        />
      )}

      {fetchState.data && fetchState.data.receipts.length > 0 && (
        <>
          <div className="card overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-ink-800 bg-ink-900/40">
                  <th className="px-4 py-2.5 text-left text-xs font-medium uppercase tracking-wider text-ink-400">
                    Outcome
                  </th>
                  <th className="px-4 py-2.5 text-left text-xs font-medium uppercase tracking-wider text-ink-400">
                    Scope
                  </th>
                  <th className="px-4 py-2.5 text-left text-xs font-medium uppercase tracking-wider text-ink-400">
                    Delegation
                  </th>
                  <th className="px-4 py-2.5 text-right text-xs font-medium uppercase tracking-wider text-ink-400">
                    When
                  </th>
                </tr>
              </thead>
              <tbody>
                {fetchState.data.receipts.map((raw, i) => (
                  <ReceiptRow key={i} raw={raw as ReceiptShape} />
                ))}
              </tbody>
            </table>
          </div>

          <Pager
            page={page}
            pageSize={PAGE_SIZE}
            total={fetchState.data.total}
            onChange={setPage}
          />
        </>
      )}
    </Page>
  );
}

function ReceiptRow({ raw }: { raw: ReceiptShape }) {
  const outcome = raw.outcome ?? "Unknown";
  const scope = raw.action?.scope_used ?? "—";
  const delegation =
    typeof raw.delegation_id === "string"
      ? raw.delegation_id
      : Array.isArray(raw.delegation_id)
        ? base64UrlFromBytes(raw.delegation_id)
        : "—";

  const executedAtIso = raw.executed_at
    ? new Date(raw.executed_at * 1000).toISOString()
    : null;

  // Slice 7: try to decode the action.value bytes as a JSON ValueClaim.
  // When present, it tells the audit story for value-bearing actions.
  const valueClaim = decodeValueClaim(raw.action?.value);

  return (
    <tr className="border-b border-ink-800/60 last:border-0 hover:bg-ink-800/30">
      <td className="px-4 py-3">
        <OutcomePill outcome={outcome} />
      </td>
      <td className="px-4 py-3">
        <div className="font-mono text-xs text-ink-300">{scope}</div>
        {valueClaim && (
          <div className="text-xs text-accent-400 mt-0.5 font-mono">
            claim: {formatClaimMoney(valueClaim.amount, valueClaim.decimals)}{" "}
            {valueClaim.currency}
          </div>
        )}
      </td>
      <td className="px-4 py-3">
        {delegation !== "—" ? (
          <CopyText value={delegation} display={shortenId(delegation, 6, 4)} />
        ) : (
          <span className="text-ink-600">—</span>
        )}
      </td>
      <td className="px-4 py-3 text-right text-ink-400" title={executedAtIso ?? ""}>
        {formatRelative(executedAtIso)}
      </td>
    </tr>
  );
}

function decodeValueClaim(
  valueB64: string | undefined,
): { currency: string; amount: number; decimals: number } | null {
  if (!valueB64) return null;
  try {
    // The receipt's action.value is base64url-encoded bytes that, for
    // value-bearing actions, contain a JSON-serialized ValueClaim.
    const padded = valueB64.replace(/-/g, "+").replace(/_/g, "/");
    const padding = "=".repeat((4 - (padded.length % 4)) % 4);
    const json = atob(padded + padding);
    const parsed = JSON.parse(json);
    if (
      typeof parsed?.currency === "string" &&
      typeof parsed?.amount === "number" &&
      typeof parsed?.decimals === "number"
    ) {
      return parsed;
    }
  } catch {
    // not a value claim; ignore
  }
  return null;
}

function formatClaimMoney(amountMinor: number, decimals: number): string {
  if (decimals === 0) return amountMinor.toString();
  const div = Math.pow(10, decimals);
  const major = Math.floor(amountMinor / div);
  const minor = (amountMinor % div).toString().padStart(decimals, "0");
  return `${major}.${minor}`;
}

function OutcomePill({ outcome }: { outcome: string }) {
  if (outcome === "Success")
    return <span className="pill-ok">success</span>;
  if (outcome === "Failure")
    return <span className="pill-bad">failure</span>;
  if (outcome === "Partial")
    return <span className="pill-warn">partial</span>;
  return <span className="pill-muted">{outcome.toLowerCase()}</span>;
}

function base64UrlFromBytes(bytes: number[]): string {
  // Receipts come back from the API with delegation_id either as a base64url
  // string or, depending on serialization, an array of byte values. Handle both.
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  const b64 = btoa(s);
  return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}

function Pager({
  page,
  pageSize,
  total,
  onChange,
}: {
  page: number;
  pageSize: number;
  total: number;
  onChange: (p: number) => void;
}) {
  const start = page * pageSize + 1;
  const end = Math.min(start + pageSize - 1, total);
  const hasNext = end < total;
  const hasPrev = page > 0;

  return (
    <div className="flex items-center justify-between text-sm text-ink-400">
      <span>
        Showing <span className="text-ink-200 tabular-nums">{start}</span>–
        <span className="text-ink-200 tabular-nums">{end}</span> of{" "}
        <span className="text-ink-200 tabular-nums">{total}</span>
      </span>
      <div className="flex gap-2">
        <button
          onClick={() => onChange(page - 1)}
          disabled={!hasPrev}
          className="btn-ghost text-xs"
        >
          Previous
        </button>
        <button
          onClick={() => onChange(page + 1)}
          disabled={!hasNext}
          className="btn-ghost text-xs"
        >
          Next
        </button>
      </div>
    </div>
  );
}
