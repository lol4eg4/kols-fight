import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "auth",
  screens: {
    login: () => import("./Login"),
    staff: () => import("./Staff"),
  },
  fills: {},
  widgets: {},
  dict,
});

export default plugin;
