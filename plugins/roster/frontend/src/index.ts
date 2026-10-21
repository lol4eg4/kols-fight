import { lazy } from "react";
import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "roster",
  screens: { network: () => import("./Network") },
  // staff only: loaded with the Collection screen, never in the client bundle
  fills: { "schedule/other_sources": lazy(() => import("./OtherSources")) },
  widgets: {},
  dict,
});

export default plugin;
