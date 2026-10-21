// Compile-time checks of the typed slot and widget contract. Not run by bun test: `bun run typecheck` compiles it.
// A line marked @ts-expect-error must fail to compile; if it starts to compile, typecheck fails.
import { lazy } from "react";
import { Slot, Widget, definePlugin } from "../src/shell";
import type { SlotProps, WidgetProps } from "../src/shell";

const base = { name: "x", screens: {}, widgets: {}, dict: {} };

// Fillers get the props the contract names.
definePlugin({ ...base, fills: { "board/rank_change": ({ date, kol }) => <span>{date + kol}</span> } });
definePlugin({ ...base, fills: { "fight/before_fight": ({ ids, date }) => <span>{ids.join() + date}</span> } });
definePlugin({ ...base, fills: { "board/network_filter": ({ onIds }) => <button onClick={() => onIds(["a"])}>x</button> } });
definePlugin({ ...base, fills: { "schedule/other_sources": () => null, "roster/wallet_status": ({ kol }) => <b>{kol}</b> } });

// A lazy filler (staff code, its own chunk) is a valid filler.
definePlugin({ ...base, fills: { "schedule/other_sources": lazy(async () => ({ default: () => <p>sources</p> })) } });
definePlugin({ ...base, fills: { "roster/wallet_status": lazy(async () => ({ default: ({ kol }: { kol: string }) => <p>{kol}</p> })) } });

// @ts-expect-error a slot that is not in the contract
definePlugin({ ...base, fills: { "board/nope": () => null } });
// @ts-expect-error the filler wants { date, kol }, not { id }
definePlugin({ ...base, fills: { "board/rank_change": (p: { id: string }) => <i>{p.id}</i> } });
// @ts-expect-error a screen loader must return a module with a default component
definePlugin({ ...base, screens: { home: async () => 5 } });
// @ts-expect-error a widget that is not offered
definePlugin({ ...base, widgets: { nothing: () => null } });
definePlugin({ ...base, fills: {}, widgets: { add_to_fight: ({ kol }) => <i>{kol}</i>, fighter_picker: () => null } });

// Hosts pass ids and dates only.
export const host = (
  <>
    <Slot name="board/rank_change" props={{ date: "2026-10-18", kol: "decu" }} />
    <Slot name="board/ticker" props={{}} />
    <Slot name="board/network_section" props={{ date: "2026-10-18" }}>
      <p>default</p>
    </Slot>
    {/* @ts-expect-error kol is missing */}
    <Slot name="board/rank_change" props={{ date: "2026-10-18" }} />
    {/* @ts-expect-error not a slot */}
    <Slot name="board/unknown" props={{}} />
    <Widget name="fight/add_to_fight" props={{ kol: "decu" }} />
    <Widget name="fight/fighter_picker" props={{}} />
    {/* @ts-expect-error not a widget */}
    <Widget name="fight/unknown" props={{}} />
  </>
);

export const a: SlotProps<"fight/before_fight"> = { ids: ["a", "b"], date: "2026-10-18" };
export const b: WidgetProps<"fight/add_to_fight"> = { kol: "decu" };
// @ts-expect-error extra props are not part of the contract
export const c: SlotProps<"board/duel_of_day"> = { date: "x", extra: 1 };
