// Constraint builder for the create-delegation form.
//
// Slice 7 exposes four protocol-level Constraint variants in the UI
// (max_value, max_rate, domain_allow, require_anchor_freshness) plus
// the gateway-side cumulative cap. The other variants (domain_deny,
// require_human_confirm, custom) are valid in the protocol but not yet
// in the UI — operators can still POST them via the JSON API if they
// need to.
//
// Currency amounts are expressed in *minor units* (cents for USD) per
// the protocol's MaxValue shape. The UI accepts decimal input and
// converts.

import { useState } from "react";
import type { Constraint, CumulativeCap } from "../api/client";

export interface ConstraintBuilderState {
  constraints: Constraint[];
  cumulativeCap: CumulativeCap | null;
}

interface Props {
  value: ConstraintBuilderState;
  onChange: (next: ConstraintBuilderState) => void;
}

type AddableKind =
  | "max_value"
  | "max_rate"
  | "domain_allow"
  | "require_anchor_freshness";

const ADDABLE: { kind: AddableKind; label: string; hint: string }[] = [
  {
    kind: "max_value",
    label: "Max per action",
    hint: "Per-action spending cap (signed into delegation).",
  },
  {
    kind: "max_rate",
    label: "Rate limit",
    hint: "Max actions in a rolling window.",
  },
  {
    kind: "domain_allow",
    label: "Domain allowlist",
    hint: "Restrict the agent to a fixed set of domains.",
  },
  {
    kind: "require_anchor_freshness",
    label: "Anchor freshness",
    hint: "Tightens the agent's anchor staleness budget.",
  },
];

function defaultFor(kind: AddableKind): Constraint {
  switch (kind) {
    case "max_value":
      return { type: "max_value", currency: "USD", amount: 5000, decimals: 2 };
    case "max_rate":
      return { type: "max_rate", count: 10, period_seconds: 60 };
    case "domain_allow":
      return { type: "domain_allow", domains: [] };
    case "require_anchor_freshness":
      return { type: "require_anchor_freshness", max_age_seconds: 60 };
  }
}

export function ConstraintBuilder({ value, onChange }: Props) {
  const [adding, setAdding] = useState<AddableKind | "">("");

  const update = (i: number, c: Constraint) => {
    const next = [...value.constraints];
    next[i] = c;
    onChange({ ...value, constraints: next });
  };

  const remove = (i: number) => {
    const next = value.constraints.filter((_, idx) => idx !== i);
    onChange({ ...value, constraints: next });
  };

  const add = (kind: AddableKind) => {
    onChange({
      ...value,
      constraints: [...value.constraints, defaultFor(kind)],
    });
    setAdding("");
  };

  return (
    <div className="space-y-4">
      <div>
        <div className="label">Constraints</div>
        {value.constraints.length === 0 && (
          <p className="text-xs text-ink-500 italic">
            No constraints yet. Add one to limit how the delegation can be used.
          </p>
        )}
        <div className="space-y-3">
          {value.constraints.map((c, i) => (
            <ConstraintRow
              key={`${c.type}-${i}`}
              constraint={c}
              onChange={(nc) => update(i, nc)}
              onRemove={() => remove(i)}
            />
          ))}
        </div>

        <div className="mt-3 flex items-center gap-2">
          <select
            className="input flex-1"
            value={adding}
            onChange={(e) => setAdding(e.target.value as AddableKind | "")}
          >
            <option value="">Add constraint…</option>
            {ADDABLE.map((a) => (
              <option key={a.kind} value={a.kind}>
                {a.label}
              </option>
            ))}
          </select>
          <button
            type="button"
            className="btn-ghost text-sm"
            disabled={!adding}
            onClick={() => adding && add(adding as AddableKind)}
          >
            Add
          </button>
        </div>
        {adding && (
          <p className="text-xs text-ink-500 mt-1.5">
            {ADDABLE.find((a) => a.kind === adding)?.hint}
          </p>
        )}
      </div>

      <CumulativeCapEditor
        value={value.cumulativeCap}
        onChange={(c) => onChange({ ...value, cumulativeCap: c })}
      />
    </div>
  );
}

function ConstraintRow({
  constraint,
  onChange,
  onRemove,
}: {
  constraint: Constraint;
  onChange: (c: Constraint) => void;
  onRemove: () => void;
}) {
  return (
    <div className="card p-3 space-y-2">
      <div className="flex items-center justify-between">
        <div className="text-xs font-mono uppercase tracking-wider text-accent-400">
          {constraint.type}
        </div>
        <button
          type="button"
          onClick={onRemove}
          className="text-xs text-ink-500 hover:text-bad-400"
        >
          remove
        </button>
      </div>
      <ConstraintFields constraint={constraint} onChange={onChange} />
    </div>
  );
}

function ConstraintFields({
  constraint,
  onChange,
}: {
  constraint: Constraint;
  onChange: (c: Constraint) => void;
}) {
  if (constraint.type === "max_value") {
    return (
      <div className="grid grid-cols-3 gap-2">
        <div>
          <label className="text-xs text-ink-500">Currency</label>
          <input
            className="input-mono"
            value={constraint.currency}
            onChange={(e) => onChange({ ...constraint, currency: e.target.value })}
          />
        </div>
        <div>
          <label className="text-xs text-ink-500">Amount (minor units)</label>
          <input
            type="number"
            min={1}
            className="input-mono"
            value={constraint.amount}
            onChange={(e) =>
              onChange({ ...constraint, amount: parseInt(e.target.value || "0", 10) })
            }
          />
        </div>
        <div>
          <label className="text-xs text-ink-500">Decimals</label>
          <input
            type="number"
            min={0}
            max={8}
            className="input-mono"
            value={constraint.decimals}
            onChange={(e) =>
              onChange({ ...constraint, decimals: parseInt(e.target.value || "0", 10) })
            }
          />
        </div>
        <p className="col-span-3 text-xs text-ink-500">
          {formatMoney(constraint.amount, constraint.decimals)} {constraint.currency}{" "}
          per action.
        </p>
      </div>
    );
  }
  if (constraint.type === "max_rate") {
    return (
      <div className="grid grid-cols-2 gap-2">
        <div>
          <label className="text-xs text-ink-500">Max actions</label>
          <input
            type="number"
            min={1}
            className="input-mono"
            value={constraint.count}
            onChange={(e) =>
              onChange({ ...constraint, count: parseInt(e.target.value || "1", 10) })
            }
          />
        </div>
        <div>
          <label className="text-xs text-ink-500">Window (seconds)</label>
          <input
            type="number"
            min={1}
            className="input-mono"
            value={constraint.period_seconds}
            onChange={(e) =>
              onChange({
                ...constraint,
                period_seconds: parseInt(e.target.value || "1", 10),
              })
            }
          />
        </div>
        <p className="col-span-2 text-xs text-ink-500">
          {constraint.count} actions per {constraint.period_seconds}s rolling
          window.
        </p>
      </div>
    );
  }
  if (constraint.type === "domain_allow") {
    return (
      <div>
        <label className="text-xs text-ink-500">
          Domains (comma-separated)
        </label>
        <input
          className="input-mono"
          value={constraint.domains.join(", ")}
          onChange={(e) =>
            onChange({
              ...constraint,
              domains: e.target.value
                .split(/[\s,]+/)
                .map((d) => d.trim())
                .filter(Boolean),
            })
          }
          placeholder="api.example.com, partner.example.com"
        />
      </div>
    );
  }
  if (constraint.type === "require_anchor_freshness") {
    return (
      <div>
        <label className="text-xs text-ink-500">Max anchor age (seconds)</label>
        <input
          type="number"
          min={1}
          className="input-mono"
          value={constraint.max_age_seconds}
          onChange={(e) =>
            onChange({
              ...constraint,
              max_age_seconds: parseInt(e.target.value || "1", 10),
            })
          }
        />
      </div>
    );
  }
  return null;
}

function CumulativeCapEditor({
  value,
  onChange,
}: {
  value: CumulativeCap | null;
  onChange: (c: CumulativeCap | null) => void;
}) {
  return (
    <div>
      <div className="flex items-center justify-between">
        <div className="label !mb-0">Cumulative cap (gateway-enforced)</div>
        <label className="flex items-center gap-2 text-xs cursor-pointer">
          <input
            type="checkbox"
            checked={value !== null}
            onChange={(e) =>
              onChange(
                e.target.checked
                  ? { currency: "USD", amount: 20000, decimals: 2 }
                  : null,
              )
            }
            className="h-4 w-4 rounded border-ink-600 bg-ink-950
                       text-accent-500 focus:ring-accent-500 focus:ring-offset-0"
          />
          <span className="text-ink-400">enable</span>
        </label>
      </div>
      <p className="text-xs text-ink-500 mt-1 mb-2">
        Total spend across all actions. Layered on top of the per-action
        cap, enforced by the gateway's ledger.
      </p>
      {value && (
        <div className="grid grid-cols-3 gap-2">
          <div>
            <label className="text-xs text-ink-500">Currency</label>
            <input
              className="input-mono"
              value={value.currency}
              onChange={(e) => onChange({ ...value, currency: e.target.value })}
            />
          </div>
          <div>
            <label className="text-xs text-ink-500">Amount (minor)</label>
            <input
              type="number"
              min={1}
              className="input-mono"
              value={value.amount}
              onChange={(e) =>
                onChange({ ...value, amount: parseInt(e.target.value || "0", 10) })
              }
            />
          </div>
          <div>
            <label className="text-xs text-ink-500">Decimals</label>
            <input
              type="number"
              min={0}
              max={8}
              className="input-mono"
              value={value.decimals}
              onChange={(e) =>
                onChange({
                  ...value,
                  decimals: parseInt(e.target.value || "0", 10),
                })
              }
            />
          </div>
          <p className="col-span-3 text-xs text-ink-500">
            Cap: {formatMoney(value.amount, value.decimals)} {value.currency}{" "}
            cumulative.
          </p>
        </div>
      )}
    </div>
  );
}

export function formatMoney(amountMinor: number, decimals: number): string {
  if (decimals === 0) return amountMinor.toString();
  const div = Math.pow(10, decimals);
  const major = Math.floor(amountMinor / div);
  const minor = (amountMinor % div).toString().padStart(decimals, "0");
  return `${major}.${minor}`;
}
