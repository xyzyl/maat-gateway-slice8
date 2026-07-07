import { jsx as _jsx, jsxs as _jsxs } from "react/jsx-runtime";
import { useState } from "react";
import { useAuth } from "../lib/auth";
import { ApiClientError } from "../api/client";
export function LoginPage() {
    const { login } = useAuth();
    const [tenant, setTenant] = useState("");
    const [email, setEmail] = useState("");
    const [password, setPassword] = useState("");
    const [error, setError] = useState(null);
    const [submitting, setSubmitting] = useState(false);
    const onSubmit = async (e) => {
        e.preventDefault();
        setError(null);
        setSubmitting(true);
        try {
            await login(tenant.trim(), email.trim(), password);
        }
        catch (err) {
            if (err instanceof ApiClientError) {
                setError(err.message);
            }
            else {
                setError("Login failed. Check the dashboard service is reachable.");
            }
        }
        finally {
            setSubmitting(false);
        }
    };
    return (_jsxs("div", { className: "min-h-screen flex items-center justify-center px-4", children: [_jsx("div", { "aria-hidden": true, className: "fixed inset-0 pointer-events-none opacity-[0.06]\n                   bg-[linear-gradient(to_right,#fff_1px,transparent_1px),\n                       linear-gradient(to_bottom,#fff_1px,transparent_1px)]\n                   bg-[size:32px_32px]" }), _jsxs("div", { className: "relative w-full max-w-sm", children: [_jsxs("div", { className: "mb-8 text-center", children: [_jsxs("div", { className: "inline-flex items-center gap-2 mb-2", children: [_jsx("span", { className: "text-3xl font-semibold tracking-tight text-ink-50", children: "maat" }), _jsx("span", { className: "px-1.5 py-0.5 rounded text-[0.65rem] font-mono uppercase\n                         tracking-wider text-accent-400 border border-accent-500/40", children: "dashboard" })] }), _jsx("p", { className: "text-sm text-ink-400", children: "sign in to manage delegations and audit receipts" })] }), _jsxs("form", { onSubmit: onSubmit, className: "card p-6 space-y-4", children: [_jsxs("div", { children: [_jsx("label", { htmlFor: "tenant", className: "label", children: "Tenant" }), _jsx("input", { id: "tenant", className: "input-mono", autoComplete: "organization", required: true, autoFocus: true, value: tenant, onChange: (e) => setTenant(e.target.value), placeholder: "acme" })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "email", className: "label", children: "Email" }), _jsx("input", { id: "email", type: "email", className: "input", autoComplete: "username", required: true, value: email, onChange: (e) => setEmail(e.target.value) })] }), _jsxs("div", { children: [_jsx("label", { htmlFor: "password", className: "label", children: "Password" }), _jsx("input", { id: "password", type: "password", className: "input", autoComplete: "current-password", required: true, value: password, onChange: (e) => setPassword(e.target.value) })] }), error && (_jsx("div", { className: "text-sm text-bad-400 bg-bad-500/10 border border-bad-500/30 rounded-md px-3 py-2", children: error })), _jsx("button", { type: "submit", disabled: submitting, className: "btn-primary w-full", children: submitting ? "Signing in…" : "Sign in" })] }), _jsxs("p", { className: "mt-6 text-center text-xs text-ink-500", children: ["New tenants are provisioned via", " ", _jsx("code", { className: "font-mono text-ink-400", children: "maat-gateway-bootstrap" })] })] })] }));
}
