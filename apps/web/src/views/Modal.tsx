import { type ReactNode, useEffect } from "react";

/// A dialog over the window. Escape, the close button and a click beside it close it; the form
/// inside brings its own heading.
export function Modal({ label, testId, onClose, children }: {
  label: string;
  testId: string;
  onClose: () => void;
  children: ReactNode;
}) {
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);
  return (
    <div className="modal-back" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={label} data-testid={testId}>
        <button type="button" className="modal-close" aria-label="Close" data-testid="modal-close" onClick={onClose}>×</button>
        {children}
      </div>
    </div>
  );
}
