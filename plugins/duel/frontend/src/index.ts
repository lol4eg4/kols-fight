import { definePlugin } from "@arena/shell";
import { DuelOfDay } from "./DuelOfDay";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "duel",
  screens: {},
  fills: { "board/duel_of_day": DuelOfDay },
  widgets: {},
  dict,
});

export default plugin;
