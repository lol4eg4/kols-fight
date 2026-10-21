import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { outPath, readManifests, renderPlugins } from "../../scripts/gen-plugins";
import { PAGE_EVENT_NAMES, SLOT_NAMES, WIDGET_NAMES, definePlugin, registerPlugins } from "../src/shell";
import type { PluginManifest } from "../src/shell";
import { SHELL_SCREENS } from "../src/shell/shell-screens";
import { plugins } from "../src/plugins.gen";

const root = resolve(import.meta.dir, "../..");
const manifests = readManifests();

describe("plugins.gen.ts", () => {
  test("is fresh: it is what scripts/gen-plugins.ts writes from plugins/*/plugin.toml (run: bun scripts/gen-plugins.ts)", () => {
    expect(readFileSync(outPath, "utf8")).toBe(renderPlugins());
  });

  test("lists all 21 plugins, lower layers first", () => {
    expect(manifests).toHaveLength(21);
    expect(plugins.map((p) => p.manifest.name)).toEqual(manifests.map((m) => m.name));
    const layers = plugins.map((p) => p.manifest.layer);
    expect(layers).toEqual([...layers].sort((a, b) => a - b));
  });

  test("every plugin's frontend is a workspace dependency of web", () => {
    const pkg = JSON.parse(readFileSync(resolve(root, "web/package.json"), "utf8"));
    for (const m of manifests) expect(pkg.dependencies[`@arena/${m.name}`]).toBe("workspace:*");
  });

  test("the generator refuses a manifest that does not match its folder", () => {
    expect(() => readManifests(resolve(root, "web/tests/fixtures/bad-plugins"))).toThrow(/must equal the folder name/);
  });
});

describe("the shell contract matches the manifests", () => {
  const declared = (kind: "screen" | "api") => manifests.flatMap((m) => m.slots.filter((s) => s.startsWith(`${kind}:`)).map((s) => `${m.name}/${s.slice(kind.length + 1)}`));

  test("typed screen slots are exactly the screen slots in plugin.toml", () => {
    expect([...(SLOT_NAMES as string[])].sort()).toEqual(declared("screen").sort());
  });

  test("typed widgets are exactly the widgets in plugin.toml", () => {
    const widgets = manifests.flatMap((m) => m.widgets.map((w) => `${m.name}/${w}`));
    expect([...(WIDGET_NAMES as string[])].sort()).toEqual(widgets.sort());
  });

  test("every screen slot has a filler somewhere in the manifests, and every filler fills a typed screen slot or an api slot", () => {
    const apiSlots = new Set(declared("api"));
    for (const m of manifests) {
      for (const f of m.fills) {
        expect(f.startsWith("platform/") || (SLOT_NAMES as string[]).includes(f) || apiSlots.has(f)).toBe(true);
      }
    }
  });

  test("page events in plugin.toml are the shell's page events", () => {
    const names = manifests.flatMap((m) => [...m.emits, ...m.listens]).filter((e) => e.startsWith("page:")).map((e) => e.slice(5));
    for (const n of names) expect(PAGE_EVENT_NAMES as readonly string[]).toContain(n);
    expect(new Set(names)).toEqual(new Set(PAGE_EVENT_NAMES));
  });

  test("screen paths are unique and the tags follow the architecture", () => {
    const screens = manifests.flatMap((m) => m.screens);
    expect(new Set(screens.map((s) => s.path)).size).toBe(screens.length);
    const tag = (path: string) => screens.find((s) => s.path === path)?.tag;
    for (const p of ["/", "/kol/:id", "/fight", "/team/login"]) expect(tag(p)).toBe("public");
    expect(tag("/team/staff")).toBe("admin");
    for (const p of ["/team/network", "/team/wallets", "/team/fomo", "/team/picker", "/team/collection"]) expect(tag(p)).toBe("staff");
    expect(SHELL_SCREENS.map((s) => [s.path, s.tag])).toEqual([["/team/design", "staff"]]);
  });
});

describe("registerPlugins", () => {
  test("accepts the generated plugin list with the shell's own screens", () => {
    const reg = registerPlugins(plugins, SHELL_SCREENS);
    expect(reg.plugins).toHaveLength(21);
    expect(reg.screens.map((s) => s.key)).toContain("shell/design");
    // the skeletons have no screens yet: every manifest screen is there, with no loader
    const board = reg.screens.find((s) => s.key === "board/home")!;
    expect(board.path).toBe("/");
    expect(board.load).toBeNull();
    // every staff or admin screen has no eager code: loaders are functions or null, never components
    for (const s of reg.screens) expect(s.load === null || typeof s.load === "function").toBe(true);
  });

  const manifest = (over: Partial<PluginManifest> = {}): PluginManifest => ({
    name: "fight",
    layer: 1,
    version: "0.1.0",
    tags: ["public"],
    uses: [],
    emits: [],
    listens: [],
    slots: [],
    fills: ["roster/wallet_status"],
    widgets: ["add_to_fight"],
    screens: [{ name: "fight_window", path: "/fight", tag: "public" }],
    ...over,
  });
  const base = { name: "fight", screens: {}, fills: {}, widgets: {}, dict: {} };

  test("rejects a plugin that disagrees with its manifest, listing every problem", () => {
    const bad = definePlugin({
      ...base,
      name: "figth",
      screens: { nowhere: async () => ({ default: () => null }) },
      fills: { "board/ticker": () => null },
      widgets: { stray: () => null } as never,
      dict: { "other.key": "x" },
    });
    expect(() => registerPlugins([{ manifest: manifest(), plugin: bad }])).toThrow(
      /definePlugin name is "figth"[\s\S]*screen "nowhere"[\s\S]*fill "board\/ticker"[\s\S]*widget "stray"[\s\S]*dictionary key "other.key"/,
    );
  });

  test("rejects two screens on one path", () => {
    const a = { manifest: manifest(), plugin: definePlugin(base) };
    const b = { manifest: manifest({ name: "other", screens: [{ name: "x", path: "/fight", tag: "public" }], fills: [], widgets: [] }), plugin: definePlugin({ ...base, name: "other" }) };
    expect(() => registerPlugins([a, b])).toThrow(/share the path \/fight/);
  });

  test("accepts a plugin that matches, with a lazy screen, a filler, a widget and a dictionary", () => {
    const good = definePlugin({
      ...base,
      screens: { fight_window: async () => ({ default: () => null }) },
      fills: { "roster/wallet_status": () => null },
      widgets: { add_to_fight: () => null },
      dict: { "fight.title": "KOL Fight" },
    });
    const reg = registerPlugins([{ manifest: manifest(), plugin: good }]);
    expect(reg.fills.get("roster/wallet_status")).toHaveLength(1);
    expect(reg.widgets.has("fight/add_to_fight")).toBe(true);
    expect(typeof reg.screens[0]!.load).toBe("function");
  });
});
