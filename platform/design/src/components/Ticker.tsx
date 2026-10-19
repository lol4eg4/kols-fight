import { useLayoutEffect, useRef, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

export interface TickerItem {
  id: string;
  /** What to show: text, or a link the caller renders. */
  content: ReactNode;
}

export interface TickerProps {
  items: TickerItem[];
  /** Names the strip for screen readers. */
  label: string;
  pauseLabel: string;
  resumeLabel: string;
}

const SPEED = 45; // pixels per second

/**
 * News strip: mono text on a sunken band, slow scroll. It stops on hover and on focus, has a pause button,
 * and under prefers-reduced-motion it does not move: the items wrap and show in full.
 */
export function Ticker({ items, label, pauseLabel, resumeLabel }: TickerProps) {
  const [paused, setPaused] = useState(false);
  const group = useRef<HTMLUListElement>(null);
  const [duration, setDuration] = useState(60);

  useLayoutEffect(() => {
    const w = group.current?.offsetWidth;
    if (w) setDuration(Math.max(20, Math.round(w / SPEED)));
  }, [items]);

  if (items.length === 0) return null;

  const list = (hidden: boolean) => (
    <ul className="ds-ticker__group" ref={hidden ? undefined : group} aria-hidden={hidden || undefined} inert={hidden || undefined}>
      {items.map((i) => (
        <li key={i.id} className="ds-ticker__item">
          {i.content}
        </li>
      ))}
    </ul>
  );

  return (
    <section className="ds-ticker" aria-label={label} data-paused={paused || undefined} style={{ "--ticker-duration": `${duration}s` } as CSSProperties}>
      <div className="ds-ticker__viewport">
        <div className="ds-ticker__track">
          {list(false)}
          {list(true)}
        </div>
      </div>
      <button type="button" className="ds-ticker__pause" aria-pressed={paused} aria-label={paused ? resumeLabel : pauseLabel} onClick={() => setPaused((p) => !p)}>
        <span aria-hidden="true">{paused ? "▶" : "❚❚"}</span>
      </button>
    </section>
  );
}
