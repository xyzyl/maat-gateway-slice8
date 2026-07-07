import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
/** Standard page layout: title row + scrollable content area. */
export function Page({ title, description, action, children }) {
    return (_jsxs("div", { className: "px-8 py-6 max-w-7xl mx-auto", children: [_jsxs("div", { className: "flex items-end justify-between mb-6 gap-6", children: [_jsxs("div", { children: [_jsx("h1", { className: "h-page", children: title }), description && (_jsx("p", { className: "text-sm text-ink-400 mt-1", children: description }))] }), action && _jsx("div", { children: action })] }), _jsx("div", { className: "space-y-6", children: children })] }));
}
