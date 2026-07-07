import { useState, type ReactNode } from "react";
import { useAuth } from "../lib/auth";
import { OverviewPage } from "../pages/OverviewPage";
import { DelegationsPage } from "../pages/DelegationsPage";
import { PrincipalKeysPage } from "../pages/PrincipalKeysPage";
import { ApiKeysPage } from "../pages/ApiKeysPage";
import { ReceiptsPage } from "../pages/ReceiptsPage";
import { UsersPage } from "../pages/UsersPage";

type PageKey =
  | "overview"
  | "delegations"
  | "principal-keys"
  | "api-keys"
  | "receipts"
  | "users";

interface NavItem {
  key: PageKey;
  label: string;
  icon: ReactNode;
}

const NAV: NavItem[] = [
  { key: "overview", label: "Overview", icon: <DotIcon /> },
  { key: "delegations", label: "Delegations", icon: <ChainIcon /> },
  { key: "principal-keys", label: "Principal keys", icon: <KeyIcon /> },
  { key: "api-keys", label: "API keys", icon: <ShieldIcon /> },
  { key: "receipts", label: "Receipts", icon: <ScrollIcon /> },
  { key: "users", label: "Users", icon: <UsersIcon /> },
];

export function Shell() {
  const { state, logout } = useAuth();
  const [page, setPage] = useState<PageKey>("overview");

  if (state.status !== "authenticated") return null;

  return (
    <div className="flex h-full">
      {/* Sidebar */}
      <aside className="w-60 shrink-0 border-r border-ink-800 bg-ink-900/40 flex flex-col">
        <div className="px-5 py-5 border-b border-ink-800">
          <div className="flex items-baseline gap-2">
            <span className="text-lg font-semibold tracking-tight text-ink-50">
              maat
            </span>
            <span className="text-[0.65rem] font-mono uppercase tracking-wider text-accent-400">
              dashboard
            </span>
          </div>
        </div>

        <nav className="flex-1 py-4 px-2 space-y-0.5">
          {NAV.map((item) => (
            <button
              key={item.key}
              onClick={() => setPage(item.key)}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-sm
                          transition-colors ${
                            page === item.key
                              ? "bg-accent-500/10 text-accent-400"
                              : "text-ink-300 hover:bg-ink-800 hover:text-ink-100"
                          }`}
            >
              <span className="text-ink-500">{item.icon}</span>
              <span>{item.label}</span>
            </button>
          ))}
        </nav>

        {/* Footer with user info */}
        <div className="px-4 py-4 border-t border-ink-800">
          <div className="text-xs text-ink-500 mb-1">Signed in as</div>
          <div className="text-sm text-ink-200 truncate" title={state.user.email}>
            {state.user.email}
          </div>
          <div className="flex items-center justify-between mt-2">
            <span
              className={
                state.user.role === "admin" ? "pill-ok" : "pill-muted"
              }
            >
              {state.user.role}
            </span>
            <button
              onClick={() => void logout()}
              className="text-xs text-ink-400 hover:text-ink-200"
            >
              Sign out
            </button>
          </div>
        </div>
      </aside>

      {/* Page content */}
      <main className="flex-1 overflow-y-auto">
        {page === "overview" && <OverviewPage />}
        {page === "delegations" && <DelegationsPage />}
        {page === "principal-keys" && <PrincipalKeysPage />}
        {page === "api-keys" && <ApiKeysPage />}
        {page === "receipts" && <ReceiptsPage />}
        {page === "users" && <UsersPage />}
      </main>
    </div>
  );
}

// ─── Icons ─────────────────────────────────────────────────────────────────
// Hand-rolled SVGs rather than an icon library — keeps the dependency
// footprint small and the visual weight consistent.

function DotIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor">
      <circle cx="8" cy="8" r="3" />
    </svg>
  );
}

function ChainIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M6 10 L10 6" />
      <path d="M4 8 a3 3 0 0 1 0 -4 l1 -1 a3 3 0 0 1 4 4 l-1 1" />
      <path d="M12 8 a3 3 0 0 1 0 4 l-1 1 a3 3 0 0 1 -4 -4 l1 -1" />
    </svg>
  );
}

function KeyIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="5" cy="11" r="2.5" />
      <path d="M7 9 L13 3" />
      <path d="M11 5 L13 7" />
    </svg>
  );
}

function ShieldIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round">
      <path d="M8 2 L13 4 V8.5 C13 11.5 11 13.5 8 14 C5 13.5 3 11.5 3 8.5 V4 Z" />
    </svg>
  );
}

function ScrollIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M3 3 H12 V12 a1 1 0 0 0 1 1 H4 a1 1 0 0 1 -1 -1 Z" />
      <path d="M5.5 6 H10" />
      <path d="M5.5 8.5 H10" />
    </svg>
  );
}

function UsersIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="6" cy="6" r="2.5" />
      <path d="M2 13 a4 4 0 0 1 8 0" />
      <circle cx="11" cy="6.5" r="2" />
      <path d="M11 9 a3 3 0 0 1 3 3" />
    </svg>
  );
}
