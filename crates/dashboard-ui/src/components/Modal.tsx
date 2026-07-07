import { useEffect, type ReactNode } from "react";

interface Props {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  /** When true, clicking the backdrop and pressing Escape are disabled.
   *  Used for the API key reveal — operator must explicitly acknowledge. */
  blocking?: boolean;
  /** Wider modal for content-heavy dialogs (delegation create, etc.). */
  wide?: boolean;
}

export function Modal({
  open,
  onClose,
  title,
  children,
  blocking = false,
  wide = false,
}: Props) {
  useEffect(() => {
    if (!open || blocking) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, blocking, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center
                 bg-ink-950/70 backdrop-blur-sm p-4"
      onClick={blocking ? undefined : onClose}
    >
      <div
        className={`card ${wide ? "max-w-2xl" : "max-w-lg"} w-full p-6 max-h-[90vh] overflow-y-auto animate-[fadeIn_120ms_ease-out]`}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="modal-title"
      >
        <h2
          id="modal-title"
          className="text-lg font-semibold text-ink-50 mb-4"
        >
          {title}
        </h2>
        {children}
      </div>
    </div>
  );
}
