import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import Radar from "./Radar";

export const plugin = definePlugin({
  name: "radar",
  screens: {},
  fills: { "board/radar_section": Radar },
  widgets: {},
  dict,
});

export default plugin;
