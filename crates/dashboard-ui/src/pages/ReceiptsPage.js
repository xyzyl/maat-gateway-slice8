import { jsx as _jsx, jsxs as _jsxs, Fragment as _Fragment } from "react/jsx-runtime";
import { useState } from "react";
import { Page } from "../components/Page";
import { CopyText } from "../components/CopyText";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { receiptsApi } from "../api/client";
import { formatRelative, shortenId } from "../lib/format";
const PAGE_SIZE = 50;
export function ReceiptsPage() {
    const [page, setPage] = useState(0);
    const [scope, setScope] = useState("");
    const [outcome, setOutcome] = useState("");
    const fetchState = useFetch(() => receiptsApi.list({
        scope: scope || undefined,
        outcome: outcome || undefined,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
    }), [page, scope, outcome]);
    const filterBar = (_jsxs("div", { className: "flex flex-wrap gap-3 items-end", children: [_jsxs("div", { className: "flex-1 min-w-[12rem]", children: [_jsx("label", { htmlFor: "r-scope", className: "label", children: "Scope filter" }), _jsx("input", { id: "r-scope", className: "input-mono", value: scope, onChange: (e) => {
                            setScope(e.target.value);
                            setPage(0);
                        }, placeholder: "any scope" })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "r-outcome", className: "label", children: "Outcome" }), _jsxs("select", { id: "r-outcome", className: "input", value: outcome, onChange: (e) => {
                            setOutcome(e.target.value);
                            setPage(0);
                        }, children: [_jsx("option", { value: "", children: "All" }), _jsx("option", { value: "Success", children: "Success" }), _jsx("option", { value: "Failure", children: "Failure" }), _jsx("option", { value: "Partial", children: "Partial" })] })] })] }));
    return (_jsxs(Page, { title: "Receipts", description: "Cryptographic record of every verification \u2014 successes and rejections both.", children: [_jsx("div", { className: "card p-4", children: filterBar }), fetchState.loading && _jsx(Loading, {}), fetchState.error && (_jsx(ErrorState, { error: fetchState.error, onRetry: fetchState.reload })), fetchState.data && fetchState.data.receipts.length === 0 && (_jsx(EmptyState, { title: page === 0 ? "No receipts yet" : "No receipts on this page", hint: page === 0
                    ? "Receipts will appear here as agents call the verification service."
                    : "Try lowering the page or relaxing your filters." })), fetchState.data && fetchState.data.receipts.length > 0 && (_jsxs(_Fragment, { children: [_jsx("div", { className: "card overflow-hidden", children: _jsxs("table", { className: "w-full text-sm", children: [_jsx("thead", { children: _jsxs("tr", { className: "border-b border-ink-800 bg-ink-900/40", children: [_jsx("th", { className: "px-4 py-2.5 text-left text-xs font-medium uppercase tracking-wider text-ink-400", children: "Outcome" }), _jsx("th", { className: "px-4 py-2.5 text-left text-xs font-medium uppercase tracking-wider text-ink-400", children: "Scope" }), _jsx("th", { className: "px-4 py-2.5 text-left text-xs font-medium uppercase tracking-wider text-ink-400", children: "Delegation" }), _jsx("th", { className: "px-4 py-2.5 text-right text-xs font-medium uppercase tracking-wider text-ink-400", children: "When" })] }) }), _jsx("tbody", { children: fetchState.data.receipts.map((raw, i) => (_jsx(ReceiptRow, { raw: raw }, i))) })] }) }), _jsx(Pager, { page: page, pageSize: PAGE_SIZE, total: fetchState.data.total, onChange: setPage })] }))] }));
}
function ReceiptRow({ raw }) {
    const outcome = raw.outcome ?? "Unknown";
    const scope = raw.action?.scope_used ?? "—";
    const delegation = typeof raw.delegation_id === "string"
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
    return (_jsxs("tr", { className: "border-b border-ink-800/60 last:border-0 hover:bg-ink-800/30", children: [_jsx("td", { className: "px-4 py-3", children: _jsx(OutcomePill, { outcome: outcome }) }), _jsxs("td", { className: "px-4 py-3", children: [_jsx("div", { className: "font-mono text-xs text-ink-300", children: scope }), valueClaim && (_jsxs("div", { className: "text-xs text-accent-400 mt-0.5 font-mono", children: ["claim: ", formatClaimMoney(valueClaim.amount, valueClaim.decimals), " ", valueClaim.currency] }))] }), _jsx("td", { className: "px-4 py-3", children: delegation !== "—" ? (_jsx(CopyText, { value: delegation, display: shortenId(delegation, 6, 4) })) : (_jsx("span", { className: "text-ink-600", children: "\u2014" })) }), _jsx("td", { className: "px-4 py-3 text-right text-ink-400", title: executedAtIso ?? "", children: formatRelative(executedAtIso) })] }));
}
function decodeValueClaim(valueB64) {
    if (!valueB64)
        return null;
    try {
        // The receipt's action.value is base64url-encoded bytes that, for
        // value-bearing actions, contain a JSON-serialized ValueClaim.
        const padded = valueB64.replace(/-/g, "+").replace(/_/g, "/");
        const padding = "=".repeat((4 - (padded.length % 4)) % 4);
        const json = atob(padded + padding);
        const parsed = JSON.parse(json);
        if (typeof parsed?.currency === "string" &&
            typeof parsed?.amount === "number" &&
            typeof parsed?.decimals === "number") {
            return parsed;
        }
    }
    catch {
        // not a value claim; ignore
    }
    return null;
}
function formatClaimMoney(amountMinor, decimals) {
    if (decimals === 0)
        return amountMinor.toString();
    const div = Math.pow(10, decimals);
    const major = Math.floor(amountMinor / div);
    const minor = (amountMinor % div).toString().padStart(decimals, "0");
    return `${major}.${minor}`;
}
function OutcomePill({ outcome }) {
    if (outcome === "Success")
        return _jsx("span", { className: "pill-ok", children: "success" });
    if (outcome === "Failure")
        return _jsx("span", { className: "pill-bad", children: "failure" });
    if (outcome === "Partial")
        return _jsx("span", { className: "pill-warn", children: "partial" });
    return _jsx("span", { className: "pill-muted", children: outcome.toLowerCase() });
}
function base64UrlFromBytes(bytes) {
    // Receipts come back from the API with delegation_id either as a base64url
    // string or, depending on serialization, an array of byte values. Handle both.
    let s = "";
    for (const b of bytes)
        s += String.fromCharCode(b);
    const b64 = btoa(s);
    return b64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
function Pager({ page, pageSize, total, onChange, }) {
    const start = page * pageSize + 1;
    const end = Math.min(start + pageSize - 1, total);
    const hasNext = end < total;
    const hasPrev = page > 0;
    return (_jsxs("div", { className: "flex items-center justify-between text-sm text-ink-400", children: [_jsxs("span", { children: ["Showing ", _jsx("span", { className: "text-ink-200 tabular-nums", children: start }), "\u2013", _jsx("span", { className: "text-ink-200 tabular-nums", children: end }), " of", " ", _jsx("span", { className: "text-ink-200 tabular-nums", children: total })] }), _jsxs("div", { className: "flex gap-2", children: [_jsx("button", { onClick: () => onChange(page - 1), disabled: !hasPrev, className: "btn-ghost text-xs", children: "Previous" }), _jsx("button", { onClick: () => onChange(page + 1), disabled: !hasNext, className: "btn-ghost text-xs", children: "Next" })] })] }));
}
