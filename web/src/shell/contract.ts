// The contract between the shell and the plugin frontends: manifest shape, screen slots and widgets with their props.
// Slot props are ids and dates only: a filler loads its own data and shows its own Skeleton, EmptyState or ErrorLine.

export type Tag = "public" | "staff" | "admin";

export interface ScreenManifest {
  name: string;
  path: string;
  tag: Tag;
}

/** One plugins/<name>/plugin.toml, as scripts/gen-plugins.ts copies it into web/src/plugins.gen.ts. */
export interface PluginManifest {
  name: string;
  layer: number;
  version: string;
  tags: Tag[];
  uses: string[];
  emits: string[];
  listens: string[];
  /** "screen:<name>" or "api:<name>". */
  slots: string[];
  /** "<plugin>/<slot>". */
  fills: string[];
  widgets: string[];
  screens: ScreenManifest[];
}

/**
 * Screen slots, as "<host plugin>/<slot>". Date is the snapshot day, "YYYY-MM-DD".
 *
 * board/network_filter: the filler tells the host the ids of the network KOLs through onIds (the leaderboard tab).
 * board/ticker: the filler adds items with useTickerItems(); `date` is absent when the frame hosts the ticker.
 */
export interface SlotPropsMap {
  "board/network_section": { date: string };
  "board/radar_section": { date: string };
  "board/network_filter": { date: string; onIds: (ids: string[]) => void };
  "board/rank_change": { date: string; kol: string };
  "board/extra_badges": { date: string; kol: string };
  "board/ticker": { date?: string };
  "board/duel_of_day": { date: string };
  "board/below_fight": { date: string };
  "fight/before_fight": { ids: string[]; date: string };
  "schedule/other_sources": Record<string, never>;
  "roster/wallet_status": { kol: string };
}

export type SlotName = keyof SlotPropsMap;
export type SlotProps<K extends SlotName> = SlotPropsMap[K];

/** Widgets, as "<owner plugin>/<widget>". The owner defines them; plugins of the same or a higher layer place them. */
export interface WidgetPropsMap {
  "fight/add_to_fight": { kol: string };
  "fight/fighter_picker": Record<string, never>;
}

export type WidgetName = keyof WidgetPropsMap;
export type WidgetProps<K extends WidgetName> = WidgetPropsMap[K];

// Runtime lists, for tests that compare the contract with the manifests. The records keep them complete:
// adding a key to a map above without listing it here is a compile error.
const slotNames: Record<SlotName, true> = {
  "board/network_section": true,
  "board/radar_section": true,
  "board/network_filter": true,
  "board/rank_change": true,
  "board/extra_badges": true,
  "board/ticker": true,
  "board/duel_of_day": true,
  "board/below_fight": true,
  "fight/before_fight": true,
  "schedule/other_sources": true,
  "roster/wallet_status": true,
};
const widgetNames: Record<WidgetName, true> = {
  "fight/add_to_fight": true,
  "fight/fighter_picker": true,
};

export const SLOT_NAMES = Object.keys(slotNames) as SlotName[];
export const WIDGET_NAMES = Object.keys(widgetNames) as WidgetName[];
