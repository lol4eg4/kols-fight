// The pending fight: KOLs picked on the board ("⚔️ Fight" on a row or a card, the Home picker) before the
// fight window opens, and the last fight played (for Rematch). One copy per page, shared by the widgets
// and the window. Not kept across page loads: a fight link carries its own fighters.

import { useSyncExternalStore } from "react";
import { emitPageEvent } from "@arena/shell";
import { addFighter, removeFighter } from "./logic";

export interface LastFight {
  ids: string[];
  date: string;
}

export interface FightStore {
  pending: string[];
  last: LastFight | null;
}

let state: FightStore = { pending: [], last: null };
const listeners = new Set<() => void>();

function set(next: FightStore) {
  state = next;
  for (const fn of [...listeners]) fn();
}

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

export function getFightStore(): FightStore {
  return state;
}

export function useFightStore(): FightStore {
  return useSyncExternalStore(subscribe, getFightStore, getFightStore);
}

/** Adds a KOL to the pending fight and tells the page. Nothing happens when it is already in or the fight is full. */
export function addPending(kol: string): boolean {
  const next = addFighter(state.pending, kol);
  if (next.length === state.pending.length) return false;
  set({ ...state, pending: next });
  emitPageEvent("fighter_added", { kol });
  return true;
}

export function removePending(kol: string): void {
  if (!state.pending.includes(kol)) return;
  set({ ...state, pending: removeFighter(state.pending, kol) });
}

/** Replaces the pending fighters, for example with a quick pick. Fighters new to the list are announced. */
export function setPending(ids: readonly string[]): void {
  let next: string[] = [];
  for (const id of ids) next = addFighter(next, id);
  const added = next.filter((k) => !state.pending.includes(k));
  set({ ...state, pending: next });
  for (const kol of added) emitPageEvent("fighter_added", { kol });
}

export function clearPending(): void {
  if (state.pending.length > 0) set({ ...state, pending: [] });
}

export function setLastFight(last: LastFight): void {
  const prev = state.last;
  if (prev && prev.date === last.date && prev.ids.join(",") === last.ids.join(",")) return;
  set({ ...state, last: { ids: [...last.ids], date: last.date } });
}

/** For tests. */
export function resetFightStore(): void {
  set({ pending: [], last: null });
}
