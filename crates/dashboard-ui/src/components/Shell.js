import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from "react";
import { useAuth } from "../lib/auth";
import { OverviewPage } from "../pages/OverviewPage";
import { DelegationsPage } from "../pages/DelegationsPage";
import { PrincipalKeysPage } from "../pages/PrincipalKeysPage";
import { ApiKeysPage } from "../pages/ApiKeysPage";
import { ReceiptsPage } from "../pages/ReceiptsPage";
import { UsersPage } from "../pages/UsersPage";
const NAV = [
    { key: "overview", label: "Overview", icon: _jsx(DotIcon, {}) },
    { key: "delegations", label: "Delegations", icon: _jsx(ChainIcon, {}) },
    { key: "principal-keys", label: "Principal keys", icon: _jsx(KeyIcon, {}) },
    { key: "api-keys", label: "API keys", icon: _jsx(ShieldIcon, {}) },
    { key: "receipts", label: "Receipts", icon: _jsx(ScrollIcon, {}) },
    { key: "users", label: "Users", icon: _jsx(UsersIcon, {}) },
];
export function Shell() {
    const { state, logout } = useAuth();
    const [page, setPage] = useState("overview");
    if (state.status !== "authenticated")
        return null;
    return (_jsxs("div", { className: "flex h-full", children: [_jsxs("aside", { className: "w-60 shrink-0 border-r border-ink-800 bg-ink-900/40 flex flex-col", children: [_jsx("div", { className: "px-5 py-5 border-b border-ink-800", children: _jsxs("div", { className: "flex items-baseline gap-2", children: [_jsx("span", { className: "text-lg font-semibold tracking-tight text-ink-50", children: "maat" }), _jsx("span", { className: "text-[0.65rem] font-mono uppercase tracking-wider text-accent-400", children: "dashboard" })] }) }), _jsx("nav", { className: "flex-1 py-4 px-2 space-y-0.5", children: NAV.map((item) => (_jsxs("button", { onClick: () => setPage(item.key), className: `w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm
                          transition-colors ${page === item.key
                                ? "bg-accent-500/10 text-accent-400"
                                : "text-ink-300 hover:bg-ink-800 hover:text-ink-100"}`, children: [_jsx("span", { className: "text-ink-500", children: item.icon }), _jsx("span", { children: item.label })] }, item.key))) }), _jsxs("div", { className: "px-4 py-4 border-t border-ink-800", children: [_jsx("div", { className: "text-xs text-ink-500 mb-1", children: "Signed in as" }), _jsx("div", { className: "text-sm text-ink-200 truncate", title: state.user.email, children: state.user.email }), _jsxs("div", { className: "flex items-center justify-between mt-2", children: [_jsx("span", { className: state.user.role === "admin" ? "pill-ok" : "pill-muted", children: state.user.role }), _jsx("button", { onClick: () => void logout(), className: "text-xs text-ink-400 hover:text-ink-200", children: "Sign out" })] })] })] }), _jsxs("main", { className: "flex-1 overflow-y-auto", children: [page === "overview" && _jsx(OverviewPage, {}), page === "delegations" && _jsx(DelegationsPage, {}), page === "principal-keys" && _jsx(PrincipalKeysPage, {}), page === "api-keys" && _jsx(ApiKeysPage, {}), page === "receipts" && _jsx(ReceiptsPage, {}), page === "users" && _jsx(UsersPage, {})] })] }));
}
// ─── Icons ─────────────────────────────────────────────────────────────────
// Hand-rolled SVGs rather than an icon library — keeps the dependency
// footprint small and the visual weight consistent.
function DotIcon() {
    return (_jsx("svg", { width: "16", height: "16", viewBox: "0 0 16 16", fill: "currentColor", children: _jsx("circle", { cx: "8", cy: "8", r: "3" }) }));
}
function ChainIcon() {
    return (_jsxs("svg", { width: "16", height: "16", viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: "1.5", strokeLinecap: "round", children: [_jsx("path", { d: "M6 10 L10 6" }), _jsx("path", { d: "M4 8 a3 3 0 0 1 0 -4 l1 -1 a3 3 0 0 1 4 4 l-1 1" }), _jsx("path", { d: "M12 8 a3 3 0 0 1 0 4 l-1 1 a3 3 0 0 1 -4 -4 l1 -1" })] }));
}
function KeyIcon() {
    return (_jsxs("svg", { width: "16", height: "16", viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: "1.5", strokeLinecap: "round", children: [_jsx("circle", { cx: "5", cy: "11", r: "2.5" }), _jsx("path", { d: "M7 9 L13 3" }), _jsx("path", { d: "M11 5 L13 7" })] }));
}
function ShieldIcon() {
    return (_jsx("svg", { width: "16", height: "16", viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: "1.5", strokeLinejoin: "round", children: _jsx("path", { d: "M8 2 L13 4 V8.5 C13 11.5 11 13.5 8 14 C5 13.5 3 11.5 3 8.5 V4 Z" }) }));
}
function ScrollIcon() {
    return (_jsxs("svg", { width: "16", height: "16", viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: "1.5", strokeLinecap: "round", children: [_jsx("path", { d: "M3 3 H12 V12 a1 1 0 0 0 1 1 H4 a1 1 0 0 1 -1 -1 Z" }), _jsx("path", { d: "M5.5 6 H10" }), _jsx("path", { d: "M5.5 8.5 H10" })] }));
}
function UsersIcon() {
    return (_jsxs("svg", { width: "16", height: "16", viewBox: "0 0 16 16", fill: "none", stroke: "currentColor", strokeWidth: "1.5", strokeLinecap: "round", children: [_jsx("circle", { cx: "6", cy: "6", r: "2.5" }), _jsx("path", { d: "M2 13 a4 4 0 0 1 8 0" }), _jsx("circle", { cx: "11", cy: "6.5", r: "2" }), _jsx("path", { d: "M11 9 a3 3 0 0 1 3 3" })] }));
}
