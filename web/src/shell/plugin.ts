import type { ComponentType } from "react";
import type { PluginManifest, SlotName, SlotProps, Tag, WidgetName, WidgetProps } from "./contract";
import { addDictionary } from "./i18n";

/** Loads a screen on demand. `() => import("./Wallets")`: every screen is its own chunk, so staff and admin code never ships to a client page. */
export type ScreenLoader = () => Promise<{ default: ComponentType }>;

/**
 * A component. A React.lazy component fits too: use it when the filler belongs to staff code, so it stays out of the client bundle.
 * (Plain ComponentType, not a union with LazyExoticComponent: a union would stop TypeScript typing inline arrow fillers.)
 */
export type SlotFill<K extends SlotName> = ComponentType<SlotProps<K>>;
export type WidgetFill<K extends WidgetName> = ComponentType<WidgetProps<K>>;

type LocalName<K> = K extends `${string}/${infer L}` ? L : never;

export interface PluginDefinition {
  /** Folder name: the same as `name` in plugin.toml. */
  name: string;
  /** Screens by name, as in plugin.toml `screens`. */
  screens: Record<string, ScreenLoader>;
  /** Fillers for screen slots, keyed "<host plugin>/<slot>", as in plugin.toml `fills`. */
  fills: { [K in SlotName]?: SlotFill<K> };
  /** Widgets this plugin offers, keyed by the plain name in plugin.toml `widgets`. */
  widgets: { [K in WidgetName as LocalName<K>]?: WidgetFill<K> };
  /** UI strings. Every key starts with "<name>.". */
  dict: Record<string, string>;
}

export interface Plugin extends PluginDefinition {
  readonly kind: "arena-plugin";
}

/** What a plugin frontend exports as `plugin`. The shell checks it against plugin.toml at start-up. */
export function definePlugin(def: PluginDefinition): Plugin {
  return Object.freeze({ ...def, kind: "arena-plugin" as const });
}

export interface RegisteredPlugin {
  manifest: PluginManifest;
  plugin: Plugin;
}

export interface ScreenEntry {
  /** "<plugin>/<screen>"; the shell's own screens use "shell/<name>". */
  key: string;
  plugin: string;
  name: string;
  path: string;
  tag: Tag;
  /** null while the plugin has not built this screen yet: the shell shows a "not built" line. */
  load: ScreenLoader | null;
}

// Props differ per slot; the typed definePlugin and <Slot> keep each filler matched to its slot.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type AnyComponent = ComponentType<any>;

export interface FillEntry {
  plugin: string;
  component: AnyComponent;
}

export interface Registry {
  plugins: RegisteredPlugin[];
  screens: ScreenEntry[];
  fills: Map<SlotName, FillEntry[]>;
  widgets: Map<WidgetName, FillEntry>;
}

const EMPTY: Registry = { plugins: [], screens: [], fills: new Map(), widgets: new Map() };
let current: Registry = EMPTY;

export function getRegistry(): Registry {
  return current;
}

/**
 * Connects plugins to the shell: checks each plugin against its manifest, merges the dictionaries,
 * and builds the screen, slot and widget tables. Throws with every problem listed when they disagree,
 * so a wrong plugin fails at start-up and in `bun test`, never on a client's screen.
 */
export function registerPlugins(list: RegisteredPlugin[], shellScreens: ScreenEntry[] = []): Registry {
  const problems: string[] = [];
  const screens: ScreenEntry[] = [...shellScreens];
  const fills = new Map<SlotName, FillEntry[]>();
  const widgets = new Map<WidgetName, FillEntry>();
  const names = new Set<string>();

  for (const { manifest, plugin } of list) {
    const who = `plugin "${manifest.name}"`;
    if (names.has(manifest.name)) problems.push(`${who} is listed twice`);
    names.add(manifest.name);
    if (plugin.name !== manifest.name) problems.push(`${who}: definePlugin name is "${plugin.name}"`);

    const screenNames = new Set(manifest.screens.map((s) => s.name));
    for (const key of Object.keys(plugin.screens)) {
      if (!screenNames.has(key)) problems.push(`${who}: screen "${key}" is not in plugin.toml`);
    }
    for (const s of manifest.screens) {
      screens.push({ key: `${manifest.name}/${s.name}`, plugin: manifest.name, name: s.name, path: s.path, tag: s.tag, load: plugin.screens[s.name] ?? null });
    }

    for (const [slot, component] of Object.entries(plugin.fills)) {
      if (!manifest.fills.includes(slot)) {
        problems.push(`${who}: fill "${slot}" is not in plugin.toml`);
        continue;
      }
      const list = fills.get(slot as SlotName) ?? [];
      list.push({ plugin: manifest.name, component: component as AnyComponent });
      fills.set(slot as SlotName, list);
    }

    for (const [local, component] of Object.entries(plugin.widgets)) {
      if (!manifest.widgets.includes(local)) {
        problems.push(`${who}: widget "${local}" is not in plugin.toml`);
        continue;
      }
      widgets.set(`${manifest.name}/${local}` as WidgetName, { plugin: manifest.name, component: component as AnyComponent });
    }

    for (const key of Object.keys(plugin.dict)) {
      if (!key.startsWith(`${manifest.name}.`)) problems.push(`${who}: dictionary key "${key}" must start with "${manifest.name}."`);
    }
  }

  const seen = new Map<string, string>();
  for (const s of screens) {
    const other = seen.get(s.path);
    if (other) problems.push(`screens ${other} and ${s.key} share the path ${s.path}`);
    seen.set(s.path, s.key);
  }

  if (problems.length > 0) throw new Error(`plugins do not match their manifests:\n- ${problems.join("\n- ")}`);

  for (const { manifest, plugin } of list) addDictionary(manifest.name, plugin.dict);
  current = { plugins: list, screens, fills, widgets };
  return current;
}
