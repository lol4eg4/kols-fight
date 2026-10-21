import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "board",
  screens: {
    home: () => import("./Home"),
    kol_card: () => import("./KolCard"),
  },
  fills: {},
  widgets: {},
  dict,
});

export default plugin;
