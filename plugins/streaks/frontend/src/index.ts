import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import { ExtraBadges } from "./ExtraBadges";

export const plugin = definePlugin({
  name: "streaks",
  screens: {},
  fills: { "board/extra_badges": ExtraBadges },
  widgets: {},
  dict,
});

export default plugin;
