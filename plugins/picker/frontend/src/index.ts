import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "picker",
  screens: { picker: () => import("./Picker") },
  fills: {},
  widgets: {},
  dict,
});

export default plugin;
