import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from "react";
import { Page } from "../components/Page";
import { DataTable } from "../components/DataTable";
import { Modal } from "../components/Modal";
import { EmptyState, ErrorState, Loading } from "../components/States";
import { useFetch } from "../lib/useFetch";
import { useAuth } from "../lib/auth";
import { usersApi } from "../api/client";
import { formatRelative } from "../lib/format";
export function UsersPage() {
    const { state } = useAuth();
    const isAdmin = state.status === "authenticated" && state.user.role === "admin";
    const myUserId = state.status === "authenticated" ? state.user.id : null;
    const fetchState = useFetch(() => usersApi.list());
    const [creating, setCreating] = useState(false);
    const columns = [
        {
            key: "email",
            header: "Email",
            render: (u) => (_jsxs("span", { className: "text-ink-100", children: [u.email, u.id === myUserId && (_jsx("span", { className: "ml-2 text-xs text-ink-500", children: "(you)" }))] })),
        },
        {
            key: "role",
            header: "Role",
            render: (u) => u.role === "admin" ? (_jsx("span", { className: "pill-ok", children: "admin" })) : (_jsx("span", { className: "pill-muted", children: "viewer" })),
        },
        {
            key: "created",
            header: "Created",
            align: "right",
            render: (u) => (_jsx("span", { className: "text-ink-400", title: u.created_at, children: formatRelative(u.created_at) })),
        },
        {
            key: "last_login",
            header: "Last login",
            align: "right",
            render: (u) => (_jsx("span", { className: u.last_login_at ? "text-ink-300" : "text-ink-600 italic", title: u.last_login_at ?? "", children: u.last_login_at ? formatRelative(u.last_login_at) : "never" })),
        },
    ];
    const action = isAdmin && (_jsx("button", { onClick: () => setCreating(true), className: "btn-primary", children: "Add user" }));
    return (_jsxs(Page, { title: "Users", description: "Dashboard accounts. Admins can manage all resources; viewers can only read.", action: action, children: [fetchState.loading && _jsx(Loading, {}), fetchState.error && (_jsx(ErrorState, { error: fetchState.error, onRetry: fetchState.reload })), fetchState.data && fetchState.data.length > 0 && (_jsx(DataTable, { columns: columns, rows: fetchState.data, rowKey: (u) => u.id, rowAction: isAdmin
                    ? (u) => u.id !== myUserId && (_jsx("button", { onClick: async () => {
                            if (!confirm(`Remove user ${u.email}?`))
                                return;
                            try {
                                await usersApi.delete(u.id);
                                fetchState.reload();
                            }
                            catch (err) {
                                alert(err instanceof Error
                                    ? err.message
                                    : "delete failed");
                            }
                        }, className: "btn-danger text-xs", children: "Remove" }))
                    : undefined })), fetchState.data && fetchState.data.length === 0 && (_jsx(EmptyState, { title: "No users." })), _jsx(Modal, { open: creating, onClose: () => setCreating(false), title: "Add user", children: _jsx(CreateUserForm, { onCancel: () => setCreating(false), onCreated: () => {
                        setCreating(false);
                        fetchState.reload();
                    } }) })] }));
}
function CreateUserForm({ onCancel, onCreated, }) {
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [role, setRole] = useState("viewer");
    const [submitting, setSubmitting] = useState(false);
    const [error, setError] = useState(null);
    const onSubmit = async (e) => {
        e.preventDefault();
        setSubmitting(true);
        setError(null);
        try {
            await usersApi.create(email.trim(), password, role);
            onCreated();
        }
        catch (err) {
            setError(err instanceof Error ? err.message : "create failed");
        }
        finally {
            setSubmitting(false);
        }
    };
    return (_jsxs("form", { onSubmit: onSubmit, className: "space-y-4", children: [_jsxs("div", { children: [_jsx("label", { htmlFor: "u-email", className: "label", children: "Email" }), _jsx("input", { id: "u-email", type: "email", className: "input", required: true, autoFocus: true, value: email, onChange: (e) => setEmail(e.target.value) })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "u-pw", className: "label", children: "Initial password" }), _jsx("input", { id: "u-pw", type: "password", className: "input", required: true, minLength: 8, value: password, onChange: (e) => setPassword(e.target.value) }), _jsx("p", { className: "text-xs text-ink-500 mt-1.5", children: "The user can change this after their first login." })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "u-role", className: "label", children: "Role" }), _jsxs("select", { id: "u-role", className: "input", value: role, onChange: (e) => setRole(e.target.value), children: [_jsx("option", { value: "viewer", children: "viewer \u2014 read-only" }), _jsx("option", { value: "admin", children: "admin \u2014 full access" })] })] }), error && (_jsx("div", { className: "text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2", children: error })), _jsxs("div", { className: "flex justify-end gap-2 pt-2", children: [_jsx("button", { type: "button", onClick: onCancel, className: "btn-ghost", disabled: submitting, children: "Cancel" }), _jsx("button", { type: "submit", className: "btn-primary", disabled: submitting, children: submitting ? "Adding…" : "Add" })] })] }));
}
