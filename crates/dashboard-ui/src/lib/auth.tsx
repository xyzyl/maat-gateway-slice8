// Auth context. Wraps the app and exposes the current user (or null).
//
// On mount, fires GET /auth/me to discover whether a session cookie is
// present. The render result tracks three states: bootstrapping, logged
// out, and logged in — so the shell can show a brief loading state on
// first paint instead of flashing the login form every page reload.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { authApi, UnauthorizedError } from "../api/client";
import type { User } from "../api/types";

type AuthState =
  | { status: "bootstrapping" }
  | { status: "anonymous" }
  | { status: "authenticated"; user: User };

interface AuthContextValue {
  state: AuthState;
  login: (tenant: string, email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  /** Force a re-bootstrap (used after a 401 mid-session). */
  invalidate: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AuthState>({ status: "bootstrapping" });
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
        if (cancelled) return;
        if (err instanceof UnauthorizedError) {
          setState({ status: "anonymous" });
        } else {
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

  const login = useCallback(
    async (tenant: string, email: string, password: string) => {
      const user = await authApi.login(tenant, email, password);
      setState({ status: "authenticated", user });
    },
    [],
  );

  const logout = useCallback(async () => {
    try {
      await authApi.logout();
    } catch {
      // logout is best-effort; if it fails we still clear local state.
    }
    setState({ status: "anonymous" });
  }, []);

  const invalidate = useCallback(() => {
    setState({ status: "bootstrapping" });
    setReloadTick((t) => t + 1);
  }, []);

  return (
    <AuthContext.Provider value={{ state, login, logout, invalidate }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used inside <AuthProvider>");
  return ctx;
}
