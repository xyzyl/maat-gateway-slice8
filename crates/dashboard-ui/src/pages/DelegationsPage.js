import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useMemo, useState } from "react";
import { Page } from "../components/Page";
import { DataTable } from "../components/DataTable";
import { CopyText } from "../components/CopyText";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { ConstraintBuilder, } from "../components/ConstraintBuilder";
import { LedgerPanel } from "../components/LedgerPanel";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { delegationsApi, principalKeysApi } from "../api/client";
import { formatRelative, shortenId, unixSeconds } from "../lib/format";
export function DelegationsPage() {
    const { state } = useAuth();
    const isAdmin = state.status === "authenticated" && state.user.role === "admin";
    const fetchState = useFetch(() => delegationsApi.list(200, 0));
    const [creating, setCreating] = useState(false);
    const [viewing, setViewing] = useState(null);
    const columns = [
        {
            key: "id",
            header: "Delegation ID",
            render: (d) => (_jsx(CopyText, { value: d.id_b64, display: shortenId(d.id_b64, 8, 6) })),
        },
        {
            key: "created",
            header: "Created",
            align: "right",
            render: (d) => (_jsx("span", { className: "text-ink-400", title: d.created_at, children: formatRelative(d.created_at) })),
        },
        {
            key: "status",
            header: "Status",
            render: (d) => d.revoked_at ? (_jsx("span", { className: "pill-bad", title: d.revocation_reason ?? "revoked", children: "revoked" })) : (_jsx("span", { className: "pill-ok", children: "active" })),
        },
    ];
    const action = isAdmin && (_jsx("button", { onClick: () => setCreating(true), className: "btn-primary", children: "Create delegation" }));
    return (_jsxs(Page, { title: "Delegations", description: "Signed authorizations granting agents the right to act within a specific scope.", action: action, children: [fetchState.loading && _jsx(Loading, {}), fetchState.error && (_jsx(ErrorState, { error: fetchState.error, onRetry: fetchState.reload })), fetchState.data?.delegations.length === 0 && (_jsx(EmptyState, { title: "No delegations yet", hint: "Create one to authorize an agent to act on behalf of your tenant.", action: action })), fetchState.data && fetchState.data.delegations.length > 0 && (_jsx(DataTable, { columns: columns, rows: fetchState.data.delegations, rowKey: (d) => d.id_b64, rowAction: (d) => (_jsxs("div", { className: "flex justify-end gap-2", children: [_jsx("button", { onClick: () => setViewing(d), className: "btn-ghost text-xs", children: "Inspect" }), isAdmin && !d.revoked_at && (_jsx("button", { onClick: async () => {
                                const reason = prompt("Reason for revocation? (optional)") ?? "";
                                if (!confirm(`Revoke delegation ${shortenId(d.id_b64, 6, 4)}?\n\nThis is permanent.`))
                                    return;
                                try {
                                    await delegationsApi.revoke(d.id_b64, reason.trim() || undefined);
                                    fetchState.reload();
                                }
                                catch (err) {
                                    alert(err instanceof Error ? err.message : "revoke failed");
                                }
                            }, className: "btn-danger text-xs", children: "Revoke" }))] })) })), _jsx(Modal, { open: creating, onClose: () => setCreating(false), title: "Create delegation", wide: true, children: _jsx(CreateDelegationForm, { onCancel: () => setCreating(false), onCreated: () => {
                        setCreating(false);
                        fetchState.reload();
                    } }) }), _jsx(Modal, { open: !!viewing, onClose: () => setViewing(null), title: "Delegation details", wide: true, children: viewing && _jsx(DelegationDetail, { d: viewing }) })] }));
}
function DelegationDetail({ d }) {
    return (_jsxs("div", { className: "space-y-4", children: [_jsxs("div", { children: [_jsx("div", { className: "label", children: "ID" }), _jsx(CopyText, { value: d.id_b64 })] }), _jsxs("div", { children: [_jsx("div", { className: "label", children: "Created" }), _jsx("p", { className: "text-sm text-ink-200", children: new Date(d.created_at).toLocaleString() })] }), d.revoked_at && (_jsxs("div", { children: [_jsx("div", { className: "label", children: "Revoked" }), _jsxs("p", { className: "text-sm text-bad-400", children: [new Date(d.revoked_at).toLocaleString(), d.revocation_reason && (_jsxs("span", { className: "text-ink-400", children: [" ", "\u2014 ", d.revocation_reason] }))] })] })), _jsxs("div", { children: [_jsx("div", { className: "label", children: "Ledger" }), _jsx(LedgerPanel, { delegationIdB64: d.id_b64 })] }), _jsxs("div", { children: [_jsx("div", { className: "label", children: "Signed delegation (JSON)" }), _jsx("pre", { className: "text-xs font-mono text-ink-300 bg-ink-950 border border-ink-800\n                     rounded-md p-3 max-h-64 overflow-auto", children: JSON.stringify(d.delegation, null, 2) })] })] }));
}
function CreateDelegationForm({ onCancel, onCreated, }) {
    const principalKeys = useFetch(() => principalKeysApi.list());
    const [principalKeyId, setPrincipalKeyId] = useState("");
    const [agentPubkey, setAgentPubkey] = useState("");
    const [scopes, setScopes] = useState("");
    // Default expiry: 1 hour from now. Operators almost always tighten this
    // up, but a short default is the safer pre-fill than something long.
    const [hoursFromNow, setHoursFromNow] = useState(1);
    const [constraintState, setConstraintState] = useState({ constraints: [], cumulativeCap: null });
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState(null);
    const activeKeys = useMemo(() => principalKeys.data?.filter((k) => !k.retired_at) ?? [], [principalKeys.data]);
    const onSubmit = async (e) => {
        e.preventDefault();
        setSubmitting(true);
        setError(null);
        try {
            const scopeArray = scopes
                .split(/[\s,]+/)
                .map((s) => s.trim())
                .filter(Boolean);
            if (scopeArray.length === 0) {
                throw new Error("At least one scope grant is required.");
            }
            const now = unixSeconds(new Date());
            const notAfter = now + Math.floor(hoursFromNow * 3600);
            await delegationsApi.create({
                principal_key_id: principalKeyId,
                agent_pubkey_b64: agentPubkey.trim(),
                scope_grants: scopeArray,
                not_before: now,
                not_after: notAfter,
                constraints: constraintState.constraints.length > 0
                    ? constraintState.constraints
                    : undefined,
                cumulative_cap: constraintState.cumulativeCap ?? undefined,
            });
            onCreated();
        }
        catch (err) {
            setError(err instanceof Error ? err.message : "create failed");
        }
        finally {
            setSubmitting(false);
        }
    };
    if (principalKeys.loading)
        return _jsx(Loading, {});
    if (activeKeys.length === 0) {
        return (_jsxs("div", { className: "space-y-4", children: [_jsx("p", { className: "text-sm text-ink-300", children: "You need at least one active principal key to create a delegation." }), _jsx("div", { className: "flex justify-end", children: _jsx("button", { onClick: onCancel, className: "btn-ghost", children: "Close" }) })] }));
    }
    return (_jsxs("form", { onSubmit: onSubmit, className: "space-y-4", children: [_jsxs("div", { children: [_jsx("label", { htmlFor: "d-pk", className: "label", children: "Principal key" }), _jsxs("select", { id: "d-pk", className: "input", value: principalKeyId, onChange: (e) => setPrincipalKeyId(e.target.value), required: true, children: [_jsx("option", { value: "", children: "Select\u2026" }), activeKeys.map((k) => (_jsx("option", { value: k.id, children: k.name }, k.id)))] })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "d-agent", className: "label", children: "Agent public key (base64url, 32 bytes)" }), _jsx("textarea", { id: "d-agent", className: "input-mono h-20 resize-none break-all", required: true, value: agentPubkey, onChange: (e) => setAgentPubkey(e.target.value), placeholder: "paste agent public key here" })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "d-scopes", className: "label", children: "Scope grants" }), _jsx("input", { id: "d-scopes", className: "input-mono", required: true, value: scopes, onChange: (e) => setScopes(e.target.value), placeholder: "finance:payment:execute, ops:read" }), _jsxs("p", { className: "text-xs text-ink-500 mt-1.5", children: ["Comma- or space-separated. Example:", " ", _jsx("code", { className: "text-ink-300", children: "test:action" })] })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "d-expiry", className: "label", children: "Expires in (hours)" }), _jsx("input", { id: "d-expiry", type: "number", min: "0.1", step: "0.1", className: "input", value: hoursFromNow, onChange: (e) => setHoursFromNow(parseFloat(e.target.value)) })] }), _jsx("div", { className: "border-t border-ink-800 pt-4", children: _jsx(ConstraintBuilder, { value: constraintState, onChange: setConstraintState }) }), error && (_jsx("div", { className: "text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2", children: error })), _jsxs("div", { className: "flex justify-end gap-2 pt-2", children: [_jsx("button", { type: "button", onClick: onCancel, className: "btn-ghost", disabled: submitting, children: "Cancel" }), _jsx("button", { type: "submit", className: "btn-primary", disabled: submitting, children: submitting ? "Signing…" : "Create & sign" })] })] }));
}
