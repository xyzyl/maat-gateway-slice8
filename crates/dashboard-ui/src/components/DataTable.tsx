import type { ReactNode } from "react";

export interface Column<T> {
  key: string;
  header: string;
  /** Render the cell. Receives the row. */
  render: (row: T) => ReactNode;
  /** Cell width hint. */
  width?: string;
  /** Right-align numeric/date columns for legibility. */
  align?: "left" | "right";
}

interface Props<T> {
  columns: Column<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  /** Optional per-row actions, rendered in the rightmost column. */
  rowAction?: (row: T) => ReactNode;
}

export function DataTable<T>({ columns, rows, rowKey, rowAction }: Props<T>) {
  return (
    <div className="card overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-ink-800 bg-ink-900/40">
            {columns.map((col) => (
              <th
                key={col.key}
                className={`px-4 py-2.5 text-xs font-medium uppercase
                            tracking-wider text-ink-400
                            ${col.align === "right" ? "text-right" : "text-left"}`}
                style={col.width ? { width: col.width } : undefined}
              >
                {col.header}
              </th>
            ))}
            {rowAction && (
              <th className="px-4 py-2.5 w-px" aria-label="actions" />
            )}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr
              key={rowKey(row)}
              className={`border-b border-ink-800/60 last:border-0
                          hover:bg-ink-800/30 transition-colors
                          ${i % 2 === 1 ? "bg-ink-900/30" : ""}`}
            >
              {columns.map((col) => (
                <td
                  key={col.key}
                  className={`px-4 py-3 text-ink-200
                              ${col.align === "right" ? "text-right" : ""}`}
                >
                  {col.render(row)}
                </td>
              ))}
              {rowAction && (
                <td className="px-4 py-3 text-right whitespace-nowrap">
                  {rowAction(row)}
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
