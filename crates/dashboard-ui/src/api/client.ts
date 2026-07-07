// Typed wrapper over fetch for the dashboard API.
//
// All session state lives in HTTP-only cookies that the browser manages
// transparently — we never see them. `credentials: "include"` makes
// fetch send them on every request.
//
// On a 401, we throw `UnauthorizedError`. The shell catches that and
// kicks the user back to the login screen.

import type {
  ApiKey,
  ApiKeyCreated,
  DelegationListResponse,
  DelegationView,
  PrincipalKey,
  ReceiptListResponse,
  Tenant,
  User,
  UserListItem,
  UserRole,
} from "./types";

const API_BASE = "/dashboard/v1";

export class ApiClientError extends Error {
  status: number;
  payload: unknown;

  constructor(status: number, message: string, payload: unknown) {
    super(message);
    this.status = status;
    this.payload = payload;
    this.name = "ApiClientError";
  }
}

export class UnauthorizedError extends ApiClientError {
  constructor(payload: unknown) {
    super(401, "unauthorized", payload);
    this.name = "UnauthorizedError";
  }
}

interface RequestOptions {
  method?: "GET" | "POST" | "DELETE";
  body?: unknown;
  query?: Record<string, string | number | undefined | null>;
  /** Skip the global 401 handling. Login uses this. */
  allowUnauthorized?: boolean;
}

async function request<T>(
  path: string,
  opts: RequestOptions = {},
): Promise<T> {
  const url = new URL(`${API_BASE}${path}`, window.location.origin);
  if (opts.query) {
    for (const [k, v] of Object.entries(opts.query)) {
      if (v !== undefined && v !== null && v !== "") {
        url.searchParams.set(k, String(v));
      }
    }
  }

  const init: RequestInit = {
    method: opts.method ?? "GET",
    credentials: "include",
    headers: opts.body
      ? { "Content-Type": "application/json" }
      : {},
  };
  if (opts.body !== undefined) {
    init.body = JSON.stringify(opts.body);
  }

  const resp = await fetch(url.pathname + url.search, init);

  // 204 No Content is common for DELETE/revoke endpoints.
  if (resp.status === 204) {
    return undefined as T;
  }

  let payload: unknown = null;
  const text = await resp.text();
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = text;
    }
  }

  if (!resp.ok) {
    if (resp.status === 401 && !opts.allowUnauthorized) {
      throw new UnauthorizedError(payload);
    }
    const message =
      typeof payload === "object" && payload && "error" in payload
        ? String((payload as { error: unknown }).error)
        : `request failed: ${resp.status}`;
    throw new ApiClientError(resp.status, message, payload);
  }

  return payload as T;
}

// ─── Auth ───
export const authApi = {
  login: (tenant: string, email: string, password: string) =>
    request<User>("/auth/login", {
      method: "POST",
      body: { tenant, email, password },
      allowUnauthorized: true,
    }),
  logout: () => request<void>("/auth/logout", { method: "POST" }),
  me: () => request<User>("/auth/me"),
};

// ─── Tenant + API keys ───
export const tenantApi = {
  current: () => request<Tenant>("/tenants/current"),
  listApiKeys: () => request<ApiKey[]>("/tenants/current/api-keys"),
  createApiKey: (name: string) =>
    request<ApiKeyCreated>("/tenants/current/api-keys", {
      method: "POST",
      body: { name },
    }),
  revokeApiKey: (id: string) =>
    request<void>(`/tenants/current/api-keys/${id}`, { method: "DELETE" }),
};

// ─── Principal keys ───
export const principalKeysApi = {
  list: () => request<PrincipalKey[]>("/principal-keys"),
  create: (name: string) =>
    request<PrincipalKey>("/principal-keys", {
      method: "POST",
      body: { name },
    }),
  retire: (id: string) =>
    request<void>(`/principal-keys/${id}/retire`, { method: "POST" }),
};

// ─── Delegations ───

// Maat protocol Constraint enum mirrored in TypeScript.
// Wire format: { type: "max_value", currency, amount, decimals }
export type Constraint =
  | { type: "max_value"; currency: string; amount: number; decimals: number }
  | { type: "max_rate"; count: number; period_seconds: number }
  | { type: "domain_allow"; domains: string[] }
  | { type: "domain_deny"; domains: string[] }
  | { type: "require_anchor_freshness"; max_age_seconds: number }
  | { type: "require_human_confirm"; threshold: string }
  | { type: "custom"; type_uri: string; value: string };

export interface CumulativeCap {
  currency: string;
  amount: number;
  decimals: number;
}

export interface CreateDelegationParams {
  principal_key_id: string;
  agent_pubkey_b64: string;
  scope_grants: string[];
  not_before?: number;
  not_after: number;
  constraints?: Constraint[];
  cumulative_cap?: CumulativeCap;
}

export interface LedgerEntry {
  receipt_id_b64: string;
  currency: string | null;
  amount: number | null;
  decimals: number | null;
  recorded_at: string;
  reversed_at: string | null;
}

export interface LedgerSummary {
  cap: CumulativeCap | null;
  recorded_total: number;
  recent_entries: LedgerEntry[];
}

export const delegationsApi = {
  list: (limit?: number, offset?: number) =>
    request<DelegationListResponse>("/delegations", {
      query: { limit, offset },
    }),
  get: (idB64: string) =>
    request<DelegationView>(`/delegations/${encodeURIComponent(idB64)}`),
  create: (params: CreateDelegationParams) =>
    request<DelegationView>("/delegations", {
      method: "POST",
      body: params,
    }),
  revoke: (idB64: string, reason?: string) =>
    request<void>(`/delegations/${encodeURIComponent(idB64)}/revoke`, {
      method: "POST",
      body: { reason },
    }),
  ledger: (idB64: string) =>
    request<LedgerSummary>(
      `/delegations/${encodeURIComponent(idB64)}/ledger`,
    ),
};

// ─── Receipts ───
// Type alias (not interface) so it gets an implicit index signature and
// remains assignable to RequestOptions["query"].
export type ReceiptFilters = {
  scope?: string;
  outcome?: string;
  since?: number;
  until?: number;
  limit?: number;
  offset?: number;
};

export const receiptsApi = {
  list: (filters: ReceiptFilters = {}) =>
    request<ReceiptListResponse>("/receipts", { query: filters }),
};

// ─── Users ───
export const usersApi = {
  list: () => request<UserListItem[]>("/users"),
  create: (email: string, password: string, role: UserRole) =>
    request<UserListItem>("/users", {
      method: "POST",
      body: { email, password, role },
    }),
  delete: (id: string) =>
    request<void>(`/users/${id}`, { method: "DELETE" }),
};
