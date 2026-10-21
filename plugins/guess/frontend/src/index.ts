import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import { Guess } from "./Guess";

export const plugin = definePlugin({
  name: "guess",
  screens: {},
  fills: { "fight/before_fight": Guess },
  widgets: {},
  dict,
});

export default plugin;
