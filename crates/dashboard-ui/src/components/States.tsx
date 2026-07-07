import type { ReactNode } from "react";

export function Loading({ label = "Loading" }: { label?: string }) {
  return (
    <div className="flex items-center justify-center py-16 text-ink-500 text-sm">
      <div className="h-2 w-2 rounded-full bg-accent-500 animate-pulse mr-2" />
      {label}…
    </div>
  );
}

export function ErrorState({
  error,
  onRetry,
}: {
  error: Error;
  onRetry?: () => void;
}) {
  return (
    <div className="card p-6 border-bad-500/30">
      <p className="text-bad-400 text-sm font-medium">Something went wrong</p>
      <p className="text-ink-400 text-sm mt-1">{error.message}</p>
      {onRetry && (
        <button onClick={onRetry} className="btn-ghost mt-4 text-xs">
          Try again
        </button>
      )}
    </div>
  );
}

export function EmptyState({
  title,
  hint,
  action,
}: {
  title: string;
  hint?: string;
  action?: ReactNode;
}) {
  return (
    <div className="card p-12 text-center">
      <p className="text-ink-300 font-medium">{title}</p>
      {hint && <p className="text-ink-500 text-sm mt-1">{hint}</p>}
      {action && <div className="mt-4 inline-block">{action}</div>}
    </div>
  );
}
