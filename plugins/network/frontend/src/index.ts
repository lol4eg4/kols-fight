import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import NetworkFilter from "./NetworkFilter";
import NetworkSection from "./NetworkSection";

export const plugin = definePlugin({
  name: "network",
  screens: {},
  fills: {
    "board/network_section": NetworkSection,
    "board/network_filter": NetworkFilter,
  },
  widgets: {},
  dict,
});

export default plugin;
