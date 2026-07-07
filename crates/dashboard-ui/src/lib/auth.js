import { jsx as _jsx } from "react/jsx-runtime";
// Auth context. Wraps the app and exposes the current user (or null).
//
// On mount, fires GET /auth/me to discover whether a session cookie is
// present. The render result tracks three states: bootstrapping, logged
// out, and logged in — so the shell can show a brief loading state on
// first paint instead of flashing the login form every page reload.
import { createContext, useCallback, useContext, useEffect, useState, } from "react";
import { authApi, UnauthorizedError } from "../api/client";
const AuthContext = createContext(null);
export function AuthProvider({ children }) {
    const [state, setState] = useState({ status: "bootstrapping" });
    const [reloadTick, setReloadTick] = useState(0);
    useEffect(() => {
        let cancelled = false;
        authApi
            .me()
            .then((user) => {
            if (!cancelled) {
                setState({ status: "authenticated", user });
            }
        })
            .catch((err) => {
            if (cancelled)
                return;
            if (err instanceof UnauthorizedError) {
                setState({ status: "anonymous" });
            }
            else {
                // Network/server error during bootstrap: treat as anonymous so
                // the user sees the login screen rather than a blank page.
                // The login attempt will surface the underlying error.
                console.error("auth bootstrap failed:", err);
                setState({ status: "anonymous" });
            }
        });
        return () => {
            cancelled = true;
        };
    }, [reloadTick]);
    const login = useCallback(async (tenant, email, password) => {
        const user = await authApi.login(tenant, email, password);
        setState({ status: "authenticated", user });
    }, []);
    const logout = useCallback(async () => {
        try {
            await authApi.logout();
        }
        catch {
            // logout is best-effort; if it fails we still clear local state.
        }
        setState({ status: "anonymous" });
    }, []);
    const invalidate = useCallback(() => {
        setState({ status: "bootstrapping" });
        setReloadTick((t) => t + 1);
    }, []);
    return (_jsx(AuthContext.Provider, { value: { state, login, logout, invalidate }, children: children }));
}
export function useAuth() {
    const ctx = useContext(AuthContext);
    if (!ctx)
        throw new Error("useAuth must be used inside <AuthProvider>");
    return ctx;
}
