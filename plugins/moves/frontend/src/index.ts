import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import { MovesTicker } from "./MovesTicker";
import { RankChange } from "./RankChange";

export const plugin = definePlugin({
  name: "moves",
  screens: {},
  fills: { "board/rank_change": RankChange, "board/ticker": MovesTicker },
  widgets: {},
  dict,
});

export default plugin;
