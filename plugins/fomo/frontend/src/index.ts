import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "fomo",
  screens: { fomo: () => import("./Fomo") },
  fills: {},
  widgets: {},
  dict,
});

export default plugin;
