// Typed wrapper over fetch for the dashboard API.
//
// All session state lives in HTTP-only cookies that the browser manages
// transparently — we never see them. `credentials: "include"` makes
// fetch send them on every request.
//
// On a 401, we throw `UnauthorizedError`. The shell catches that and
// kicks the user back to the login screen.
const API_BASE = "/dashboard/v1";
export class ApiClientError extends Error {
    status;
    payload;
    constructor(status, message, payload) {
        super(message);
        this.status = status;
        this.payload = payload;
        this.name = "ApiClientError";
    }
}
export class UnauthorizedError extends ApiClientError {
    constructor(payload) {
        super(401, "unauthorized", payload);
        this.name = "UnauthorizedError";
    }
}
async function request(path, opts = {}) {
    const url = new URL(`${API_BASE}${path}`, window.location.origin);
    if (opts.query) {
        for (const [k, v] of Object.entries(opts.query)) {
            if (v !== undefined && v !== null && v !== "") {
                url.searchParams.set(k, String(v));
            }
        }
    }
    const init = {
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
        return undefined;
    }
    let payload = null;
    const text = await resp.text();
    if (text) {
        try {
            payload = JSON.parse(text);
        }
        catch {
            payload = text;
        }
    }
    if (!resp.ok) {
        if (resp.status === 401 && !opts.allowUnauthorized) {
            throw new UnauthorizedError(payload);
        }
        const message = typeof payload === "object" && payload && "error" in payload
            ? String(payload.error)
            : `request failed: ${resp.status}`;
        throw new ApiClientError(resp.status, message, payload);
    }
    return payload;
}
// ─── Auth ───
export const authApi = {
    login: (tenant, email, password) => request("/auth/login", {
        method: "POST",
        body: { tenant, email, password },
        allowUnauthorized: true,
    }),
    logout: () => request("/auth/logout", { method: "POST" }),
    me: () => request("/auth/me"),
};
// ─── Tenant + API keys ───
export const tenantApi = {
    current: () => request("/tenants/current"),
    listApiKeys: () => request("/tenants/current/api-keys"),
    createApiKey: (name) => request("/tenants/current/api-keys", {
        method: "POST",
        body: { name },
    }),
    revokeApiKey: (id) => request(`/tenants/current/api-keys/${id}`, { method: "DELETE" }),
};
// ─── Principal keys ───
export const principalKeysApi = {
    list: () => request("/principal-keys"),
    create: (name) => request("/principal-keys", {
        method: "POST",
        body: { name },
    }),
    retire: (id) => request(`/principal-keys/${id}/retire`, { method: "POST" }),
};
export const delegationsApi = {
    list: (limit, offset) => request("/delegations", {
        query: { limit, offset },
    }),
    get: (idB64) => request(`/delegations/${encodeURIComponent(idB64)}`),
    create: (params) => request("/delegations", {
        method: "POST",
        body: params,
    }),
    revoke: (idB64, reason) => request(`/delegations/${encodeURIComponent(idB64)}/revoke`, {
        method: "POST",
        body: { reason },
    }),
    ledger: (idB64) => request(`/delegations/${encodeURIComponent(idB64)}/ledger`),
};
export const receiptsApi = {
    list: (filters = {}) => request("/receipts", { query: filters }),
};
// ─── Users ───
export const usersApi = {
    list: () => request("/users"),
    create: (email, password, role) => request("/users", {
        method: "POST",
        body: { email, password, role },
    }),
    delete: (id) => request(`/users/${id}`, { method: "DELETE" }),
};
