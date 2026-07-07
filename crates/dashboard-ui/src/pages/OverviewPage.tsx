import { useFetch } from "../lib/useFetch";
import {
  delegationsApi,
  principalKeysApi,
  receiptsApi,
  tenantApi,
} from "../api/client";
import { Page } from "../components/Page";
import { Loading } from "../components/States";

export function OverviewPage() {
  const tenant = useFetch(() => tenantApi.current());
  const principalKeys = useFetch(() => principalKeysApi.list());
  const delegations = useFetch(() => delegationsApi.list(1000, 0));
  const receipts = useFetch(() => receiptsApi.list({ limit: 1 }));

  if (tenant.loading) return <Loading />;

  const activeKeys =
    principalKeys.data?.filter((k) => !k.retired_at).length ?? 0;
  const totalDelegations = delegations.data?.delegations.length ?? 0;
  const activeDelegations =
    delegations.data?.delegations.filter((d) => !d.revoked_at).length ?? 0;
  const revokedDelegations = totalDelegations - activeDelegations;
  const totalReceipts = receipts.data?.total ?? 0;

  return (
    <Page
      title={tenant.data?.name ?? "Overview"}
      description={
        tenant.data
          ? `Tenant slug: ${tenant.data.slug}`
          : "Loading tenant info"
      }
    >
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <Stat
          label="Active principal keys"
          value={activeKeys}
          loading={principalKeys.loading}
        />
        <Stat
          label="Active delegations"
          value={activeDelegations}
          loading={delegations.loading}
          accent
        />
        <Stat
          label="Revoked delegations"
          value={revokedDelegations}
          loading={delegations.loading}
          tone={revokedDelegations > 0 ? "warn" : "muted"}
        />
        <Stat
          label="Total receipts"
          value={totalReceipts}
          loading={receipts.loading}
        />
      </div>

      <div className="card p-6">
        <h2 className="h-section mb-3">Getting started</h2>
        <ol className="space-y-2 text-sm text-ink-300 list-decimal list-inside">
          <li>
            Create a{" "}
            <strong className="text-ink-100">principal key</strong> — used to
            sign delegations on behalf of your tenant.
          </li>
          <li>
            Create a{" "}
            <strong className="text-ink-100">delegation</strong> for an agent
            (you'll need its public key).
          </li>
          <li>
            Hand the signed delegation to the agent. It uses the delegation
            plus a signed anchor to call the gateway's{" "}
            <code className="font-mono text-xs text-ink-200">
              POST /v1/verify
            </code>{" "}
            endpoint.
          </li>
          <li>
            Verified actions produce receipts, which appear on the{" "}
            <strong className="text-ink-100">Receipts</strong> page.
          </li>
        </ol>
      </div>
    </Page>
  );
}

function Stat({
  label,
  value,
  loading,
  accent,
  tone,
}: {
  label: string;
  value: number | string;
  loading?: boolean;
  accent?: boolean;
  tone?: "warn" | "muted";
}) {
  const toneClass = accent
    ? "text-accent-400"
    : tone === "warn"
      ? "text-warn-400"
      : "text-ink-50";

  return (
    <div className="card p-5">
      <div className="text-xs uppercase tracking-wider text-ink-400 mb-2">
        {label}
      </div>
      <div className={`text-3xl font-semibold tabular-nums ${toneClass}`}>
        {loading ? "—" : value}
      </div>
    </div>
  );
}
