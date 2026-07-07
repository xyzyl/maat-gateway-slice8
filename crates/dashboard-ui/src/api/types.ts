// TypeScript types matching the dashboard service's response shapes.
//
// These mirror the Rust structs in `crates/dashboard/src/handlers/*.rs`.
// If the API changes, this is the place to keep the UI in sync.

export type UserRole = "admin" | "viewer";

export interface User {
  id: string;
  tenant_id: string;
  email: string;
  role: UserRole;
}

export interface UserListItem {
  id: string;
  email: string;
  role: UserRole;
  created_at: string;
  last_login_at: string | null;
}

export interface Tenant {
  id: string;
  slug: string;
  name: string;
}

export interface ApiKey {
  id: string;
  name: string;
  key_prefix: string;
  created_at: string;
  last_used_at: string | null;
  revoked_at: string | null;
}

export interface ApiKeyCreated {
  metadata: ApiKey;
  full_key: string;
}

export interface PrincipalKey {
  id: string;
  name: string;
  kms_key_id: string;
  public_key_b64: string;
  created_at: string;
  retired_at: string | null;
}

export interface DelegationView {
  id_b64: string;
  delegation: unknown;
  created_at: string;
  revoked_at: string | null;
  revocation_reason: string | null;
}

export interface DelegationListResponse {
  delegations: DelegationView[];
}

export interface ReceiptListResponse {
  receipts: unknown[];
  total: number;
}

export interface ApiError {
  error: string;
}
