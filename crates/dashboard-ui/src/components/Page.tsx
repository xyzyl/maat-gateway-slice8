import type { ReactNode } from "react";

interface Props {
  title: string;
  description?: string;
  action?: ReactNode;
  children: ReactNode;
}

/** Standard page layout: title row + scrollable content area. */
export function Page({ title, description, action, children }: Props) {
  return (
    <div className="px-8 py-6 max-w-7xl mx-auto">
      <div className="flex items-end justify-between mb-6 gap-6">
        <div>
          <h1 className="h-page">{title}</h1>
          {description && (
            <p className="text-sm text-ink-400 mt-1">{description}</p>
          )}
        </div>
        {action && <div>{action}</div>}
      </div>
      <div className="space-y-6">{children}</div>
    </div>
  );
}
