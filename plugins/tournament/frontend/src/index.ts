import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import { Tournament } from "./Tournament";

export const plugin = definePlugin({
  name: "tournament",
  screens: {},
  fills: { "board/below_fight": Tournament },
  widgets: {},
  dict,
});

export default plugin;
