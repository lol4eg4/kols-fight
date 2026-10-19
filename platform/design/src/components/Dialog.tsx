import { useEffect, useId, useRef } from "react";
import type { ReactNode, SyntheticEvent } from "react";
import { cx } from "../cx";

export interface DialogProps {
  open: boolean;
  /** Called by the close button, Esc and a click on the dim background. The caller closes it. */
  onClose: () => void;
  /** Names the dialog. Drawn at the top unless titleHidden. */
  title: string;
  titleHidden?: boolean;
  /** Text of the close button, for screen readers. */
  closeLabel: string;
  /** Pinned bottom bar, for example "Add to fight" on a phone. */
  footer?: ReactNode;
  /** Frame colour: coin for a card, pink for the fight window. */
  tone?: "coin" | "pink";
  children: ReactNode;
}

/**
 * Modal window on the native dialog element: the rest of the page is inert, Tab stays inside,
 * Esc closes it, and focus goes back to what opened it. Full screen on a phone.
 */
export function Dialog({ open, onClose, title, titleHidden, closeLabel, footer, tone = "coin", children }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const opener = useRef<HTMLElement | null>(null);
  const titleId = useId();

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      d.showModal();
    } else if (!open && d.open) {
      d.close();
      restoreFocus(opener);
    }
  }, [open]);

  // Leaving while open (the screen unmounts): close and give focus back.
  useEffect(() => {
    const d = ref.current;
    return () => {
      if (d?.open) d.close();
      restoreFocus(opener);
    };
  }, []);

  const cancel = (e: SyntheticEvent) => {
    e.preventDefault(); // we close it ourselves, so the caller's state stays the truth
    onClose();
  };

  return (
    <dialog
      ref={ref}
      className={cx("ds-dialog", `ds-dialog--${tone}`)}
      aria-labelledby={titleId}
      onCancel={cancel}
      onClick={(e) => {
        if (e.target === ref.current) onClose(); // the dim background
      }}
    >
      <div className="ds-dialog__panel">
        <div className="ds-dialog__head">
          <h2 id={titleId} className={cx("ds-dialog__title", titleHidden && "ds-sr-only")}>
            {title}
          </h2>
          <button type="button" className="ds-dialog__close" onClick={onClose} aria-label={closeLabel}>
            <span aria-hidden="true">×</span>
          </button>
        </div>
        <div className="ds-dialog__body">{children}</div>
        {footer && <div className="ds-dialog__footer">{footer}</div>}
      </div>
    </dialog>
  );
}

function restoreFocus(opener: { current: HTMLElement | null }) {
  const el = opener.current;
  opener.current = null;
  if (el?.isConnected) el.focus();
}
