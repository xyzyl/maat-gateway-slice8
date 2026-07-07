// Formatting helpers shared across the dashboard.

/** "5m ago", "2h ago", "3d ago", "Apr 14" */
export function formatRelative(iso: string | null | undefined): string {
  if (!iso) return "—";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "—";
  const diffMs = Date.now() - then;
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 5) return "just now";
  if (diffSec < 60) return `${diffSec}s ago`;
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  if (diffSec < 86400 * 7) return `${Math.floor(diffSec / 86400)}d ago`;

  // Older than a week → absolute month-day.
  return new Date(iso).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

/** ISO timestamp formatted for tooltip display. */
export function formatAbsolute(iso: string | null | undefined): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString();
}

/** Truncate a long base64url ID for display. Keeps 6 + ellipsis + 4. */
export function shortenId(id: string, head = 6, tail = 4): string {
  if (id.length <= head + tail + 1) return id;
  return `${id.slice(0, head)}…${id.slice(-tail)}`;
}

/** Best-effort copy to clipboard. Resolves true on success. */
export async function copyToClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Convert a JS Date to Unix seconds. */
export function unixSeconds(d: Date): number {
  return Math.floor(d.getTime() / 1000);
}
