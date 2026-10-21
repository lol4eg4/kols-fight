import type { ScreenEntry } from "./plugin";

/**
 * Screens the shell owns, registered here and not by a plugin. The gallery is staff-only and lazy-loaded:
 * it lands in its own chunk, which a client page never loads.
 */
export const SHELL_SCREENS: ScreenEntry[] = [
  {
    key: "shell/design",
    plugin: "shell",
    name: "design",
    path: "/team/design",
    tag: "staff",
    load: () => import("./gallery/Gallery"),
  },
];
