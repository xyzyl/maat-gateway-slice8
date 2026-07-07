import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
export function Loading({ label = "Loading" }) {
    return (_jsxs("div", { className: "flex items-center justify-center py-16 text-ink-500 text-sm", children: [_jsx("div", { className: "h-2 w-2 rounded-full bg-accent-500 animate-pulse mr-2" }), label, "\u2026"] }));
}
export function ErrorState({ error, onRetry, }) {
    return (_jsxs("div", { className: "card p-6 border-bad-500/30", children: [_jsx("p", { className: "text-bad-400 text-sm font-medium", children: "Something went wrong" }), _jsx("p", { className: "text-ink-400 text-sm mt-1", children: error.message }), onRetry && (_jsx("button", { onClick: onRetry, className: "btn-ghost mt-4 text-xs", children: "Try again" }))] }));
}
export function EmptyState({ title, hint, action, }) {
    return (_jsxs("div", { className: "card p-12 text-center", children: [_jsx("p", { className: "text-ink-300 font-medium", children: title }), hint && _jsx("p", { className: "text-ink-500 text-sm mt-1", children: hint }), action && _jsx("div", { className: "mt-4 inline-block", children: action })] }));
}
