import { definePlugin } from "@arena/shell";
import { dict } from "./en";
import { AddToFight, FighterPicker } from "./Widgets";
import "./fight.css";

export const plugin = definePlugin({
  name: "fight",
  screens: {
    fight_window: () => import("./FightWindow"),
  },
  fills: {},
  widgets: {
    add_to_fight: AddToFight,
    fighter_picker: FighterPicker,
  },
  dict,
});

export default plugin;
