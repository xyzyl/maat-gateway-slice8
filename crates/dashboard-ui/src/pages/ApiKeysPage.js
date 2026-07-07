import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from "react";
import { Page } from "../components/Page";
import { DataTable } from "../components/DataTable";
import { CopyText } from "../components/CopyText";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { tenantApi } from "../api/client";
import { formatRelative } from "../lib/format";
export function ApiKeysPage() {
    const { state } = useAuth();
    const isAdmin = state.status === "authenticated" && state.user.role === "admin";
    const fetchState = useFetch(() => tenantApi.listApiKeys());
    const [creating, setCreating] = useState(false);
    const [revealed, setRevealed] = useState(null);
    const columns = [
        {
            key: "name",
            header: "Name",
            render: (k) => _jsx("span", { className: "text-ink-100", children: k.name }),
        },
        {
            key: "prefix",
            header: "Key",
            render: (k) => (_jsxs("span", { className: "font-mono text-sm text-ink-300", children: ["mgw_live_", k.key_prefix, _jsx("span", { className: "text-ink-600", children: "\u2026" })] })),
        },
        {
            key: "created",
            header: "Created",
            align: "right",
            render: (k) => (_jsx("span", { className: "text-ink-400", title: k.created_at, children: formatRelative(k.created_at) })),
        },
        {
            key: "last_used",
            header: "Last used",
            align: "right",
            render: (k) => (_jsx("span", { className: k.last_used_at ? "text-ink-300" : "text-ink-600 italic", title: k.last_used_at ?? "", children: k.last_used_at ? formatRelative(k.last_used_at) : "never" })),
        },
        {
            key: "status",
            header: "Status",
            render: (k) => k.revoked_at ? (_jsx("span", { className: "pill-bad", children: "revoked" })) : (_jsx("span", { className: "pill-ok", children: "active" })),
        },
    ];
    const action = isAdmin && (_jsx("button", { onClick: () => setCreating(true), className: "btn-primary", children: "Create API key" }));
    return (_jsxs(Page, { title: "API keys", description: "Bearer credentials used by agents and integrations to call the verification service.", action: action, children: [fetchState.loading && _jsx(Loading, {}), fetchState.error && (_jsx(ErrorState, { error: fetchState.error, onRetry: fetchState.reload })), fetchState.data && fetchState.data.length === 0 && (_jsx(EmptyState, { title: "No API keys yet", hint: "Create one to authenticate calls to the verification service.", action: action })), fetchState.data && fetchState.data.length > 0 && (_jsx(DataTable, { columns: columns, rows: fetchState.data, rowKey: (k) => k.id, rowAction: isAdmin
                    ? (k) => !k.revoked_at && (_jsx("button", { onClick: async () => {
                            if (!confirm(`Revoke API key "${k.name}"?\n\nAny agent or integration using this key will start receiving 401.`))
                                return;
                            await tenantApi.revokeApiKey(k.id);
                            fetchState.reload();
                        }, className: "btn-danger text-xs", children: "Revoke" }))
                    : undefined })), _jsx(Modal, { open: creating, onClose: () => setCreating(false), title: "Create API key", children: _jsx(CreateForm, { onCancel: () => setCreating(false), onCreated: (created) => {
                        setCreating(false);
                        setRevealed(created);
                        fetchState.reload();
                    } }) }), _jsx(Modal, { open: !!revealed, onClose: () => setRevealed(null), title: "API key created", blocking: true, children: revealed && (_jsx(RevealForm, { created: revealed, onAcknowledge: () => setRevealed(null) })) })] }));
}
function CreateForm({ onCancel, onCreated, }) {
    const [name, setName] = useState("");
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState(null);
    const onSubmit = async (e) => {
        e.preventDefault();
        setSubmitting(true);
        setError(null);
        try {
            const created = await tenantApi.createApiKey(name.trim());
            onCreated(created);
        }
        catch (err) {
            setError(err instanceof Error ? err.message : "create failed");
        }
        finally {
            setSubmitting(false);
        }
    };
    return (_jsxs("form", { onSubmit: onSubmit, className: "space-y-4", children: [_jsxs("div", { children: [_jsx("label", { htmlFor: "ak-name", className: "label", children: "Name" }), _jsx("input", { id: "ak-name", className: "input", value: name, onChange: (e) => setName(e.target.value), required: true, autoFocus: true, placeholder: "production-agent" })] }), error && (_jsx("div", { className: "text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2", children: error })), _jsxs("div", { className: "flex justify-end gap-2 pt-2", children: [_jsx("button", { type: "button", onClick: onCancel, className: "btn-ghost", disabled: submitting, children: "Cancel" }), _jsx("button", { type: "submit", className: "btn-primary", disabled: submitting, children: submitting ? "Creating…" : "Create" })] })] }));
}
function RevealForm({ created, onAcknowledge, }) {
    const [acknowledged, setAcknowledged] = useState(false);
    return (_jsxs("div", { className: "space-y-5", children: [_jsxs("div", { className: "bg-warn-500/10 border border-warn-500/30 rounded-md p-3", children: [_jsx("p", { className: "text-sm text-warn-400 font-medium", children: "This is the only time the full key will be shown." }), _jsx("p", { className: "text-xs text-ink-300 mt-1", children: "If you lose it, you'll need to revoke this key and create a new one." })] }), _jsxs("div", { children: [_jsx("div", { className: "label", children: created.metadata.name }), _jsx("div", { className: "bg-ink-950 border border-ink-700 rounded-md p-3", children: _jsx(CopyText, { value: created.full_key, display: created.full_key, className: "break-all w-full text-xs" }) })] }), _jsxs("label", { className: "flex items-start gap-2 text-sm cursor-pointer", children: [_jsx("input", { type: "checkbox", checked: acknowledged, onChange: (e) => setAcknowledged(e.target.checked), className: "mt-0.5 h-4 w-4 rounded border-ink-600 bg-ink-950\n                     text-accent-500 focus:ring-accent-500 focus:ring-offset-0" }), _jsx("span", { className: "text-ink-300", children: "I've copied this key somewhere safe." })] }), _jsx("div", { className: "flex justify-end", children: _jsx("button", { onClick: onAcknowledge, disabled: !acknowledged, className: "btn-primary", children: "Done" }) })] }));
}
