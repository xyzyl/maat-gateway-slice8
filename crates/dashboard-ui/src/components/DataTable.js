import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
export function DataTable({ columns, rows, rowKey, rowAction }) {
    return (_jsx("div", { className: "card overflow-hidden", children: _jsxs("table", { className: "w-full text-sm", children: [_jsx("thead", { children: _jsxs("tr", { className: "border-b border-ink-800 bg-ink-900/40", children: [columns.map((col) => (_jsx("th", { className: `px-4 py-2.5 text-xs font-medium uppercase
                            tracking-wider text-ink-400
                            ${col.align === "right" ? "text-right" : "text-left"}`, style: col.width ? { width: col.width } : undefined, children: col.header }, col.key))), rowAction && (_jsx("th", { className: "px-4 py-2.5 w-px", "aria-label": "actions" }))] }) }), _jsx("tbody", { children: rows.map((row, i) => (_jsxs("tr", { className: `border-b border-ink-800/60 last:border-0
                          hover:bg-ink-800/30 transition-colors
                          ${i % 2 === 1 ? "bg-ink-900/30" : ""}`, children: [columns.map((col) => (_jsx("td", { className: `px-4 py-3 text-ink-200
                              ${col.align === "right" ? "text-right" : ""}`, children: col.render(row) }, col.key))), rowAction && (_jsx("td", { className: "px-4 py-3 text-right whitespace-nowrap", children: rowAction(row) }))] }, rowKey(row)))) })] }) }));
}
