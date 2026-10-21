import { useMemo } from "react";
import type { ReactNode } from "react";
import { Link, t, useTickerItems } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { useMoves } from "./moves";
import type { Climb } from "./moves";

const NONE: { id: string; content: ReactNode }[] = [];

function item(id: string, climb: Climb, text: (vars: { name: string; n: number }) => string) {
  return {
    id,
    content: <Link to={`/kol/${encodeURIComponent(climb.kol)}`}>{text({ name: climb.name ?? climb.kol, n: climb.places })}</Link>,
  };
}

/** Adds «Climb of the week» (and of the day) to the news strip. It draws nothing itself. */
export function MovesTicker({ date }: SlotProps<"board/ticker">) {
  const state = useMoves(date);
  const data = state.status === "ready" ? state.data : null;
  const items = useMemo(() => {
    if (!data) return NONE;
    const out = [];
    if (data.week_climb) out.push(item("week", data.week_climb, (v) => t("moves.ticker.week", v)));
    if (data.day_climb) out.push(item("day", data.day_climb, (v) => t("moves.ticker.day", v)));
    return out;
  }, [data]);
  useTickerItems("moves", items);
  return null;
}

export default MovesTicker;
