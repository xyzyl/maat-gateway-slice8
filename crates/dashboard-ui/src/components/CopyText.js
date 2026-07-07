import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from "react";
import { copyToClipboard } from "../lib/format";
/**
 * Renders text with a click-to-copy affordance. Briefly flashes "copied"
 * on success.
 */
export function CopyText({ value, display, className = "", ariaLabel }) {
    const [copied, setCopied] = useState(false);
    const handleCopy = async () => {
        const ok = await copyToClipboard(value);
        if (ok) {
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
        }
    };
    return (_jsxs("button", { type: "button", onClick: handleCopy, title: value, "aria-label": ariaLabel ?? `Copy ${value}`, className: `group inline-flex items-center gap-1.5 font-mono text-sm
                  text-ink-300 hover:text-accent-400 transition-colors ${className}`, children: [_jsx("span", { children: display ?? value }), _jsx("span", { className: `text-[0.65rem] uppercase tracking-wider transition-opacity ${copied
                    ? "opacity-100 text-ok-400"
                    : "opacity-0 group-hover:opacity-60"}`, children: copied ? "copied" : "copy" })] }));
}
