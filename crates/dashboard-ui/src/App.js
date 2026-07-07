import { jsx as _jsx } from "react/jsx-runtime";
import { AuthProvider, useAuth } from "./lib/auth";
import { LoginPage } from "./pages/LoginPage";
import { Shell } from "./components/Shell";
export function App() {
    return (_jsx(AuthProvider, { children: _jsx(AppRouter, {}) }));
}
function AppRouter() {
    const { state } = useAuth();
    if (state.status === "bootstrapping") {
        return (_jsx("div", { className: "min-h-screen flex items-center justify-center", children: _jsx("div", { className: "h-2 w-2 rounded-full bg-accent-500 animate-pulse" }) }));
    }
    if (state.status === "anonymous") {
        return _jsx(LoginPage, {});
    }
    return _jsx(Shell, {});
}
