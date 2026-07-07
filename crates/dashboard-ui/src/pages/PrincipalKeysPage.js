import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from "react";
import { Page } from "../components/Page";
import { DataTable } from "../components/DataTable";
import { CopyText } from "../components/CopyText";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { principalKeysApi } from "../api/client";
import { formatRelative, shortenId } from "../lib/format";
export function PrincipalKeysPage() {
    const { state } = useAuth();
    const isAdmin = state.status === "authenticated" && state.user.role === "admin";
    const fetchState = useFetch(() => principalKeysApi.list());
    const [creating, setCreating] = useState(false);
    const columns = [
        {
            key: "name",
            header: "Name",
            render: (k) => _jsx("span", { className: "text-ink-100", children: k.name }),
        },
        {
            key: "kms_key_id",
            header: "KMS key id",
            render: (k) => (_jsx(CopyText, { value: k.kms_key_id, display: shortenId(k.kms_key_id, 8, 6) })),
        },
        {
            key: "public_key",
            header: "Public key",
            render: (k) => (_jsx(CopyText, { value: k.public_key_b64, display: shortenId(k.public_key_b64, 10, 6) })),
        },
        {
            key: "created",
            header: "Created",
            align: "right",
            render: (k) => (_jsx("span", { className: "text-ink-400", title: k.created_at, children: formatRelative(k.created_at) })),
        },
        {
            key: "status",
            header: "Status",
            render: (k) => k.retired_at ? (_jsx("span", { className: "pill-muted", children: "retired" })) : (_jsx("span", { className: "pill-ok", children: "active" })),
        },
    ];
    const action = isAdmin && (_jsx("button", { onClick: () => setCreating(true), className: "btn-primary", children: "Create principal key" }));
    return (_jsxs(Page, { title: "Principal keys", description: "Tenant signing keys used to authorize agents via delegations. Key material lives in the KMS.", action: action, children: [fetchState.loading && _jsx(Loading, {}), fetchState.error && (_jsx(ErrorState, { error: fetchState.error, onRetry: fetchState.reload })), fetchState.data && fetchState.data.length === 0 && (_jsx(EmptyState, { title: "No principal keys yet", hint: "Create one to start signing delegations.", action: action })), fetchState.data && fetchState.data.length > 0 && (_jsx(DataTable, { columns: columns, rows: fetchState.data, rowKey: (k) => k.id, rowAction: isAdmin
                    ? (k) => !k.retired_at && (_jsx("button", { onClick: async () => {
                            if (!confirm(`Retire principal key "${k.name}"?`))
                                return;
                            await principalKeysApi.retire(k.id);
                            fetchState.reload();
                        }, className: "btn-ghost text-xs", children: "Retire" }))
                    : undefined })), _jsx(Modal, { open: creating, onClose: () => setCreating(false), title: "Create principal key", children: _jsx(CreateForm, { onCancel: () => setCreating(false), onCreated: () => {
                        setCreating(false);
                        fetchState.reload();
                    } }) })] }));
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
            await principalKeysApi.create(name.trim());
            onCreated();
        }
        catch (err) {
            setError(err instanceof Error ? err.message : "create failed");
        }
        finally {
            setSubmitting(false);
        }
    };
    return (_jsxs("form", { onSubmit: onSubmit, className: "space-y-4", children: [_jsxs("div", { children: [_jsx("label", { htmlFor: "pk-name", className: "label", children: "Name" }), _jsx("input", { id: "pk-name", className: "input", value: name, onChange: (e) => setName(e.target.value), required: true, autoFocus: true, placeholder: "primary" }), _jsx("p", { className: "text-xs text-ink-500 mt-1.5", children: "Internal label only. The KMS generates the underlying key." })] }), error && (_jsx("div", { className: "text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2", children: error })), _jsxs("div", { className: "flex justify-end gap-2 pt-2", children: [_jsx("button", { type: "button", onClick: onCancel, className: "btn-ghost", disabled: submitting, children: "Cancel" }), _jsx("button", { type: "submit", className: "btn-primary", disabled: submitting, children: submitting ? "Creating…" : "Create" })] })] }));
}
