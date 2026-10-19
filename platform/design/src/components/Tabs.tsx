import { useRef } from "react";
import type { KeyboardEvent, ReactNode } from "react";
import { cx } from "../cx";

export interface TabItem {
  id: string;
  label: string;
  /** Small count after the label, for example how many KOLs the tab holds. */
  count?: number | string;
  disabled?: boolean;
}

export interface TabsProps {
  items: TabItem[];
  value: string;
  onChange: (id: string) => void;
  /** Names the tab list. */
  label: string;
  /** Prefix of the element ids; pass the same to TabPanel. */
  idBase: string;
}

const tabId = (base: string, id: string) => `${base}-tab-${id}`;
const panelId = (base: string, id: string) => `${base}-panel-${id}`;

/** Pill tabs. Arrow keys, Home and End move between tabs; the selected tab is the only tab stop. */
export function Tabs({ items, value, onChange, label, idBase }: TabsProps) {
  const refs = useRef(new Map<string, HTMLButtonElement>());
  const enabled = items.filter((i) => !i.disabled);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const i = enabled.findIndex((t) => t.id === value);
    let next: number | null = null;
    if (e.key === "ArrowRight" || e.key === "ArrowDown") next = (i + 1) % enabled.length;
    else if (e.key === "ArrowLeft" || e.key === "ArrowUp") next = (i - 1 + enabled.length) % enabled.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = enabled.length - 1;
    if (next === null) return;
    e.preventDefault();
    const t = enabled[next]!;
    onChange(t.id);
    refs.current.get(t.id)?.focus();
  };

  return (
    <div className="ds-tabs" role="tablist" aria-label={label} onKeyDown={onKeyDown}>
      {items.map((t) => {
        const selected = t.id === value;
        return (
          <button
            key={t.id}
            ref={(el) => {
              if (el) refs.current.set(t.id, el);
              else refs.current.delete(t.id);
            }}
            type="button"
            role="tab"
            id={tabId(idBase, t.id)}
            aria-selected={selected}
            aria-controls={panelId(idBase, t.id)}
            tabIndex={selected ? 0 : -1}
            disabled={t.disabled}
            className={cx("ds-tab", selected && "ds-tab--active")}
            onClick={() => onChange(t.id)}
          >
            {t.label}
            {t.count !== undefined && <span className="ds-tab__count">{t.count}</span>}
          </button>
        );
      })}
    </div>
  );
}

export interface TabPanelProps {
  idBase: string;
  /** The tab this panel belongs to. */
  tab: string;
  /** The selected tab. The panel renders only when they match. */
  value: string;
  children: ReactNode;
}

export function TabPanel({ idBase, tab, value, children }: TabPanelProps) {
  if (tab !== value) return null;
  return (
    <div role="tabpanel" id={panelId(idBase, tab)} aria-labelledby={tabId(idBase, tab)} tabIndex={0} className="ds-tabpanel">
      {children}
    </div>
  );
}
