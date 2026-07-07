import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useFetch } from "../lib/useFetch";
import { delegationsApi, principalKeysApi, receiptsApi, tenantApi, } from "../api/client";
import { Page } from "../components/Page";
import { Loading } from "../components/States";
export function OverviewPage() {
    const tenant = useFetch(() => tenantApi.current());
    const principalKeys = useFetch(() => principalKeysApi.list());
    const delegations = useFetch(() => delegationsApi.list(1000, 0));
    const receipts = useFetch(() => receiptsApi.list({ limit: 1 }));
    if (tenant.loading)
        return _jsx(Loading, {});
    const activeKeys = principalKeys.data?.filter((k) => !k.retired_at).length ?? 0;
    const totalDelegations = delegations.data?.delegations.length ?? 0;
    const activeDelegations = delegations.data?.delegations.filter((d) => !d.revoked_at).length ?? 0;
    const revokedDelegations = totalDelegations - activeDelegations;
    const totalReceipts = receipts.data?.total ?? 0;
    return (_jsxs(Page, { title: tenant.data?.name ?? "Overview", description: tenant.data
            ? `Tenant slug: ${tenant.data.slug}`
            : "Loading tenant info", children: [_jsxs("div", { className: "grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4", children: [_jsx(Stat, { label: "Active principal keys", value: activeKeys, loading: principalKeys.loading }), _jsx(Stat, { label: "Active delegations", value: activeDelegations, loading: delegations.loading, accent: true }), _jsx(Stat, { label: "Revoked delegations", value: revokedDelegations, loading: delegations.loading, tone: revokedDelegations > 0 ? "warn" : "muted" }), _jsx(Stat, { label: "Total receipts", value: totalReceipts, loading: receipts.loading })] }), _jsxs("div", { className: "card p-6", children: [_jsx("h2", { className: "h-section mb-3", children: "Getting started" }), _jsxs("ol", { className: "space-y-2 text-sm text-ink-300 list-decimal list-inside", children: [_jsxs("li", { children: ["Create a", " ", _jsx("strong", { className: "text-ink-100", children: "principal key" }), " \u2014 used to sign delegations on behalf of your tenant."] }), _jsxs("li", { children: ["Create a", " ", _jsx("strong", { className: "text-ink-100", children: "delegation" }), " for an agent (you'll need its public key)."] }), _jsxs("li", { children: ["Hand the signed delegation to the agent. It uses the delegation plus a signed anchor to call the gateway's", " ", _jsx("code", { className: "font-mono text-xs text-ink-200", children: "POST /v1/verify" }), " ", "endpoint."] }), _jsxs("li", { children: ["Verified actions produce receipts, which appear on the", " ", _jsx("strong", { className: "text-ink-100", children: "Receipts" }), " page."] })] })] })] }));
}
function Stat({ label, value, loading, accent, tone, }) {
    const toneClass = accent
        ? "text-accent-400"
        : tone === "warn"
            ? "text-warn-400"
            : "text-ink-50";
    return (_jsxs("div", { className: "card p-5", children: [_jsx("div", { className: "text-xs uppercase tracking-wider text-ink-400 mb-2", children: label }), _jsx("div", { className: `text-3xl font-semibold tabular-nums ${toneClass}`, children: loading ? "—" : value })] }));
}
