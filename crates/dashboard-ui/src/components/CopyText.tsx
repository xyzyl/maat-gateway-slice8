import { useState } from "react";
import { copyToClipboard } from "../lib/format";

interface Props {
  value: string;
  /** Visible text — defaults to a shortened form of `value`. */
  display?: string;
  className?: string;
  ariaLabel?: string;
}

/**
 * Renders text with a click-to-copy affordance. Briefly flashes "copied"
 * on success.
 */
export function CopyText({ value, display, className = "", ariaLabel }: Props) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    const ok = await copyToClipboard(value);
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    }
  };

  return (
    <button
      type="button"
      onClick={handleCopy}
      title={value}
      aria-label={ariaLabel ?? `Copy ${value}`}
      className={`group inline-flex items-center gap-1.5 font-mono text-sm
                  text-ink-300 hover:text-accent-400 transition-colors ${className}`}
    >
      <span>{display ?? value}</span>
      <span
        className={`text-[0.65rem] uppercase tracking-wider transition-opacity ${
          copied
            ? "opacity-100 text-ok-400"
            : "opacity-0 group-hover:opacity-60"
        }`}
      >
        {copied ? "copied" : "copy"}
      </span>
    </button>
  );
}
