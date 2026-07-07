import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useEffect } from "react";
export function Modal({ open, onClose, title, children, blocking = false, wide = false, }) {
    useEffect(() => {
        if (!open || blocking)
            return;
        const onKey = (e) => {
            if (e.key === "Escape")
                onClose();
        };
        window.addEventListener("keydown", onKey);
        return () => window.removeEventListener("keydown", onKey);
    }, [open, blocking, onClose]);
    if (!open)
        return null;
    return (_jsx("div", { className: "fixed inset-0 z-50 flex items-center justify-center\n                 bg-ink-950/70 backdrop-blur-sm p-4", onClick: blocking ? undefined : onClose, children: _jsxs("div", { className: `card ${wide ? "max-w-2xl" : "max-w-lg"} w-full p-6 max-h-[90vh] overflow-y-auto animate-[fadeIn_120ms_ease-out]`, onClick: (e) => e.stopPropagation(), role: "dialog", "aria-modal": "true", "aria-labelledby": "modal-title", children: [_jsx("h2", { id: "modal-title", className: "text-lg font-semibold text-ink-50 mb-4", children: title }), children] }) }));
}
