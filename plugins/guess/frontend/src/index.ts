import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "guess",
  screens: {},
  fills: {},
  widgets: {},
  dict,
});

export default plugin;
