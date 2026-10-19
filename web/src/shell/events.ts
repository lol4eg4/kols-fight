import { useEffect, useRef } from "react";

// Page events: plugins on one page act on each other through named events with fixed fields.
// No plugin imports another. The set below is the contract; plugin.toml lists them as "page:<name>".

export interface PageEventMap {
  /** A KOL was added to the fight. Emitted by fight, heard by anyone. */
  fighter_added: { kol: string };
  /** A fight finished. winner is the KOL id, or null for a draw. */
  fight_over: { ids: string[]; date: string; winner: string | null };
}

export type PageEventName = keyof PageEventMap;

/** Every page event name, for tests and for checking plugin.toml. */
export const PAGE_EVENT_NAMES = ["fighter_added", "fight_over"] as const satisfies readonly PageEventName[];

type Handler<K extends PageEventName> = (payload: PageEventMap[K]) => void;

const handlers = new Map<PageEventName, Set<Handler<PageEventName>>>();

/** Calls every listener now, in the order they subscribed. A listener that throws is logged and does not stop the rest. */
export function emitPageEvent<K extends PageEventName>(name: K, payload: PageEventMap[K]): void {
  for (const h of [...(handlers.get(name) ?? [])]) {
    try {
      (h as Handler<K>)(payload);
    } catch (e) {
      console.error(`page event "${name}" listener failed`, e);
    }
  }
}

/** Subscribes outside React. Returns the unsubscribe function. */
export function onPageEvent<K extends PageEventName>(name: K, handler: Handler<K>): () => void {
  let set = handlers.get(name);
  if (!set) handlers.set(name, (set = new Set()));
  set.add(handler as Handler<PageEventName>);
  return () => set.delete(handler as Handler<PageEventName>);
}

/** Listens while the component is mounted. The latest handler is always the one called. */
export function usePageEvent<K extends PageEventName>(name: K, handler: Handler<K>): void {
  const latest = useRef(handler);
  latest.current = handler;
  useEffect(() => onPageEvent(name, (payload) => latest.current(payload)), [name]);
}
