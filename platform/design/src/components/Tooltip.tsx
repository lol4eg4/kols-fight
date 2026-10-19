import { useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { cx } from "../cx";

export interface TooltipProps {
  /** The explanation. Plain text or light markup. */
  content: ReactNode;
  /**
   * The trigger. It is wrapped in a focusable span, so keyboard focus opens the tip.
   * Set focusable={false} when the child is itself a button or link: focus inside it still opens the tip.
   */
  children: ReactNode;
  focusable?: boolean;
  /** Extra class for the trigger span (inline by default). */
  inline?: boolean;
}

const MARGIN = 8;

/** Explanation that opens on hover, on keyboard focus and on tap. Esc and a tap elsewhere close it. */
export function Tooltip({ content, children, focusable = true, inline = true }: TooltipProps) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  const trigger = useRef<HTMLSpanElement>(null);
  const tip = useRef<HTMLSpanElement>(null);
  // Pointer type of the press in progress. A tap focuses the trigger and then clicks it: the focus
  // must not open the tip a second time, or the click would close it again.
  const pressed = useRef<string | null>(null);

  const place = useCallback(() => {
    const t = trigger.current;
    const p = tip.current;
    if (!t || !p) return;
    const r = t.getBoundingClientRect();
    const w = p.offsetWidth;
    const h = p.offsetHeight;
    const vw = document.documentElement.clientWidth;
    let left = r.left + r.width / 2 - w / 2;
    left = Math.max(MARGIN, Math.min(left, vw - w - MARGIN));
    let top = r.top - h - 6;
    if (top < MARGIN) top = r.bottom + 6;
    setPos({ left, top });
  }, []);

  useLayoutEffect(() => {
    if (open) place();
    else setPos(null);
  }, [open, place, content]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault(); // the first Esc closes the tip only, not the dialog the tip sits in
        setOpen(false);
      }
    };
    const onDown = (e: PointerEvent) => {
      if (!trigger.current?.contains(e.target as Node)) setOpen(false);
    };
    const onMove = () => place();
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    window.addEventListener("scroll", onMove, true);
    window.addEventListener("resize", onMove);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onDown);
      window.removeEventListener("scroll", onMove, true);
      window.removeEventListener("resize", onMove);
    };
  }, [open, place]);

  return (
    <span
      ref={trigger}
      className={cx("ds-tip", inline && "ds-tip--inline", focusable && "ds-tip--focusable")}
      tabIndex={focusable ? 0 : undefined}
      aria-describedby={open ? id : undefined}
      onPointerEnter={(e) => e.pointerType === "mouse" && setOpen(true)}
      onPointerLeave={(e) => e.pointerType === "mouse" && setOpen(false)}
      onPointerDown={(e) => {
        pressed.current = e.pointerType;
        window.setTimeout(() => (pressed.current = null), 600);
      }}
      onFocus={() => {
        if (!pressed.current) setOpen(true);
      }}
      onBlur={() => setOpen(false)}
      onClick={() => {
        if (pressed.current === "mouse") setOpen(true);
        else setOpen((o) => !o);
      }}
    >
      {children}
      {open &&
        createPortal(
          <span
            ref={tip}
            id={id}
            role="tooltip"
            className="ds-tip__bubble"
            style={pos ? { left: pos.left, top: pos.top } : { left: 0, top: 0, visibility: "hidden" }}
          >
            {content}
          </span>,
          // Inside a modal dialog the page behind is inert and below it: the tip must live in the dialog.
          trigger.current?.closest("dialog") ?? document.body,
        )}
    </span>
  );
}
