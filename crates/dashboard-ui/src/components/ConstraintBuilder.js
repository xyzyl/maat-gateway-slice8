import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
// Constraint builder for the create-delegation form.
//
// Slice 7 exposes four protocol-level Constraint variants in the UI
// (max_value, max_rate, domain_allow, require_anchor_freshness) plus
// the gateway-side cumulative cap. The other variants (domain_deny,
// require_human_confirm, custom) are valid in the protocol but not yet
// in the UI — operators can still POST them via the JSON API if they
// need to.
//
// Currency amounts are expressed in *minor units* (cents for USD) per
// the protocol's MaxValue shape. The UI accepts decimal input and
// converts.
import { useState } from "react";
const ADDABLE = [
    {
        kind: "max_value",
        label: "Max per action",
        hint: "Per-action spending cap (signed into delegation).",
    },
    {
        kind: "max_rate",
        label: "Rate limit",
        hint: "Max actions in a rolling window.",
    },
    {
        kind: "domain_allow",
        label: "Domain allowlist",
        hint: "Restrict the agent to a fixed set of domains.",
    },
    {
        kind: "require_anchor_freshness",
        label: "Anchor freshness",
        hint: "Tightens the agent's anchor staleness budget.",
    },
];
function defaultFor(kind) {
    switch (kind) {
        case "max_value":
            return { type: "max_value", currency: "USD", amount: 5000, decimals: 2 };
        case "max_rate":
            return { type: "max_rate", count: 10, period_seconds: 60 };
        case "domain_allow":
            return { type: "domain_allow", domains: [] };
        case "require_anchor_freshness":
            return { type: "require_anchor_freshness", max_age_seconds: 60 };
    }
}
export function ConstraintBuilder({ value, onChange }) {
    const [adding, setAdding] = useState("");
    const update = (i, c) => {
        const next = [...value.constraints];
        next[i] = c;
        onChange({ ...value, constraints: next });
    };
    const remove = (i) => {
        const next = value.constraints.filter((_, idx) => idx !== i);
        onChange({ ...value, constraints: next });
    };
    const add = (kind) => {
        onChange({
            ...value,
            constraints: [...value.constraints, defaultFor(kind)],
        });
        setAdding("");
    };
    return (_jsxs("div", { className: "space-y-4", children: [_jsxs("div", { children: [_jsx("div", { className: "label", children: "Constraints" }), value.constraints.length === 0 && (_jsx("p", { className: "text-xs text-ink-500 italic", children: "No constraints yet. Add one to limit how the delegation can be used." })), _jsx("div", { className: "space-y-3", children: value.constraints.map((c, i) => (_jsx(ConstraintRow, { constraint: c, onChange: (nc) => update(i, nc), onRemove: () => remove(i) }, `${c.type}-${i}`))) }), _jsxs("div", { className: "mt-3 flex items-center gap-2", children: [_jsxs("select", { className: "input flex-1", value: adding, onChange: (e) => setAdding(e.target.value), children: [_jsx("option", { value: "", children: "Add constraint\u2026" }), ADDABLE.map((a) => (_jsx("option", { value: a.kind, children: a.label }, a.kind)))] }), _jsx("button", { type: "button", className: "btn-ghost text-sm", disabled: !adding, onClick: () => adding && add(adding), children: "Add" })] }), adding && (_jsx("p", { className: "text-xs text-ink-500 mt-1.5", children: ADDABLE.find((a) => a.kind === adding)?.hint }))] }), _jsx(CumulativeCapEditor, { value: value.cumulativeCap, onChange: (c) => onChange({ ...value, cumulativeCap: c }) })] }));
}
function ConstraintRow({ constraint, onChange, onRemove, }) {
    return (_jsxs("div", { className: "card p-3 space-y-2", children: [_jsxs("div", { className: "flex items-center justify-between", children: [_jsx("div", { className: "text-xs font-mono uppercase tracking-wider text-accent-400", children: constraint.type }), _jsx("button", { type: "button", onClick: onRemove, className: "text-xs text-ink-500 hover:text-bad-400", children: "remove" })] }), _jsx(ConstraintFields, { constraint: constraint, onChange: onChange })] }));
}
function ConstraintFields({ constraint, onChange, }) {
    if (constraint.type === "max_value") {
        return (_jsxs("div", { className: "grid grid-cols-3 gap-2", children: [_jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Currency" }), _jsx("input", { className: "input-mono", value: constraint.currency, onChange: (e) => onChange({ ...constraint, currency: e.target.value }) })] }), _jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Amount (minor units)" }), _jsx("input", { type: "number", min: 1, className: "input-mono", value: constraint.amount, onChange: (e) => onChange({ ...constraint, amount: parseInt(e.target.value || "0", 10) }) })] }), _jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Decimals" }), _jsx("input", { type: "number", min: 0, max: 8, className: "input-mono", value: constraint.decimals, onChange: (e) => onChange({ ...constraint, decimals: parseInt(e.target.value || "0", 10) }) })] }), _jsxs("p", { className: "col-span-3 text-xs text-ink-500", children: [formatMoney(constraint.amount, constraint.decimals), " ", constraint.currency, " ", "per action."] })] }));
    }
    if (constraint.type === "max_rate") {
        return (_jsxs("div", { className: "grid grid-cols-2 gap-2", children: [_jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Max actions" }), _jsx("input", { type: "number", min: 1, className: "input-mono", value: constraint.count, onChange: (e) => onChange({ ...constraint, count: parseInt(e.target.value || "1", 10) }) })] }), _jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Window (seconds)" }), _jsx("input", { type: "number", min: 1, className: "input-mono", value: constraint.period_seconds, onChange: (e) => onChange({
                                ...constraint,
                                period_seconds: parseInt(e.target.value || "1", 10),
                            }) })] }), _jsxs("p", { className: "col-span-2 text-xs text-ink-500", children: [constraint.count, " actions per ", constraint.period_seconds, "s rolling window."] })] }));
    }
    if (constraint.type === "domain_allow") {
        return (_jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Domains (comma-separated)" }), _jsx("input", { className: "input-mono", value: constraint.domains.join(", "), onChange: (e) => onChange({
                        ...constraint,
                        domains: e.target.value
                            .split(/[\s,]+/)
                            .map((d) => d.trim())
                            .filter(Boolean),
                    }), placeholder: "api.example.com, partner.example.com" })] }));
    }
    if (constraint.type === "require_anchor_freshness") {
        return (_jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Max anchor age (seconds)" }), _jsx("input", { type: "number", min: 1, className: "input-mono", value: constraint.max_age_seconds, onChange: (e) => onChange({
                        ...constraint,
                        max_age_seconds: parseInt(e.target.value || "1", 10),
                    }) })] }));
    }
    return null;
}
function CumulativeCapEditor({ value, onChange, }) {
    return (_jsxs("div", { children: [_jsxs("div", { className: "flex items-center justify-between", children: [_jsx("div", { className: "label !mb-0", children: "Cumulative cap (gateway-enforced)" }), _jsxs("label", { className: "flex items-center gap-2 text-xs cursor-pointer", children: [_jsx("input", { type: "checkbox", checked: value !== null, onChange: (e) => onChange(e.target.checked
                                    ? { currency: "USD", amount: 20000, decimals: 2 }
                                    : null), className: "h-4 w-4 rounded border-ink-600 bg-ink-950\n                       text-accent-500 focus:ring-accent-500 focus:ring-offset-0" }), _jsx("span", { className: "text-ink-400", children: "enable" })] })] }), _jsx("p", { className: "text-xs text-ink-500 mt-1 mb-2", children: "Total spend across all actions. Layered on top of the per-action cap, enforced by the gateway's ledger." }), value && (_jsxs("div", { className: "grid grid-cols-3 gap-2", children: [_jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Currency" }), _jsx("input", { className: "input-mono", value: value.currency, onChange: (e) => onChange({ ...value, currency: e.target.value }) })] }), _jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Amount (minor)" }), _jsx("input", { type: "number", min: 1, className: "input-mono", value: value.amount, onChange: (e) => onChange({ ...value, amount: parseInt(e.target.value || "0", 10) }) })] }), _jsxs("div", { children: [_jsx("label", { className: "text-xs text-ink-500", children: "Decimals" }), _jsx("input", { type: "number", min: 0, max: 8, className: "input-mono", value: value.decimals, onChange: (e) => onChange({
                                    ...value,
                                    decimals: parseInt(e.target.value || "0", 10),
                                }) })] }), _jsxs("p", { className: "col-span-3 text-xs text-ink-500", children: ["Cap: ", formatMoney(value.amount, value.decimals), " ", value.currency, " ", "cumulative."] })] }))] }));
}
export function formatMoney(amountMinor, decimals) {
    if (decimals === 0)
        return amountMinor.toString();
    const div = Math.pow(10, decimals);
    const major = Math.floor(amountMinor / div);
    const minor = (amountMinor % div).toString().padStart(decimals, "0");
    return `${major}.${minor}`;
}
