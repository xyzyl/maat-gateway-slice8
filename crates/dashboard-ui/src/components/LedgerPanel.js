import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
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
export function LedgerPanel({ delegationIdB64 }) {
    const { data, error, loading } = useFetch(() => delegationsApi.ledger(delegationIdB64), [delegationIdB64]);
    if (loading)
        return _jsx(Loading, { label: "Loading ledger" });
    if (error) {
        return (_jsxs("p", { className: "text-xs text-bad-400", children: ["Could not load ledger: ", error.message] }));
    }
    if (!data)
        return null;
    if (!data.cap && data.recent_entries.length === 0) {
        return (_jsx("p", { className: "text-xs text-ink-500 italic", children: "No cumulative cap set; no ledger activity yet." }));
    }
    const used = data.recorded_total;
    const remaining = data.cap ? Math.max(0, data.cap.amount - used) : null;
    const pct = data.cap ? Math.min(100, (used / data.cap.amount) * 100) : 0;
    return (_jsxs("div", { className: "space-y-3", children: [data.cap && (_jsxs("div", { children: [_jsxs("div", { className: "flex items-baseline justify-between", children: [_jsx("span", { className: "text-xs uppercase tracking-wider text-ink-400", children: "Cumulative spend" }), _jsxs("span", { className: "font-mono text-sm tabular-nums text-ink-200", children: [formatMoney(used, data.cap.decimals), " /", " ", formatMoney(data.cap.amount, data.cap.decimals), " ", data.cap.currency] })] }), _jsx("div", { className: "mt-1.5 h-1.5 bg-ink-800 rounded-full overflow-hidden", children: _jsx("div", { className: `h-full ${pct >= 90
                                ? "bg-bad-500"
                                : pct >= 70
                                    ? "bg-warn-500"
                                    : "bg-accent-500"}`, style: { width: `${pct}%` } }) }), remaining !== null && (_jsxs("p", { className: "text-xs text-ink-500 mt-1", children: [formatMoney(remaining, data.cap.decimals), " ", data.cap.currency, " ", "remaining"] }))] })), data.recent_entries.length > 0 && (_jsxs("div", { children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-ink-400 mb-1.5", children: "Recent activity" }), _jsx("div", { className: "space-y-1", children: data.recent_entries.map((e) => (_jsxs("div", { className: "flex items-center justify-between text-xs font-mono", children: [_jsx("span", { className: e.reversed_at ? "text-ink-600 line-through" : "text-ink-300", children: e.amount !== null && e.decimals !== null
                                        ? `${formatMoney(e.amount, e.decimals)} ${e.currency ?? ""}`
                                        : "—" }), _jsx("span", { className: "text-ink-500", title: e.recorded_at, children: formatRelative(e.recorded_at) })] }, e.receipt_id_b64))) })] }))] }));
}
