import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "schedule",
  screens: { collection: () => import("./Collection") },
  fills: {},
  widgets: {},
  dict,
});

export default plugin;
