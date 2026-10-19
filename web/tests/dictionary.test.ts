import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { dict as shellDict } from "../src/shell/en";
import { walk } from "./imports-lib";

const root = resolve(import.meta.dir, "../..");
const pluginNames = readdirSync(resolve(root, "plugins")).filter((n) => existsSync(resolve(root, "plugins", n, "plugin.toml")));

const dicts = new Map<string, Record<string, string>>([["shell", shellDict]]);
for (const n of pluginNames) {
  const mod = (await import(`../../plugins/${n}/frontend/src/en.ts`)) as { dict: Record<string, string> };
  dicts.set(n, mod.dict);
}
const all = new Set([...dicts.values()].flatMap((d) => Object.keys(d)));

/** The prefixes that mark a string literal as a dictionary key. */
const PREFIXES = ["shell", "gallery", ...pluginNames];
const KEY_LITERAL = new RegExp(`["'\`]((?:${PREFIXES.join("|")})\\.[a-z0-9_]+(?:\\.[a-z0-9_]+)*)["'\`]`, "g");

function sources(): string[] {
  const files = [...walk(resolve(root, "web/src"), [".ts", ".tsx"])];
  for (const n of pluginNames) files.push(...walk(resolve(root, "plugins", n, "frontend/src"), [".ts", ".tsx"]));
  return files.filter((f) => !f.endsWith("plugins.gen.ts") && !f.endsWith("/en.ts") && !f.endsWith("/shell/i18n.ts"));
}

describe("dictionary", () => {
  test("keys of each dictionary start with the owner's prefix", () => {
    for (const [owner, dict] of dicts) {
      for (const key of Object.keys(dict)) {
        const ok = owner === "shell" ? key.startsWith("shell.") || key.startsWith("gallery.") : key.startsWith(`${owner}.`);
        expect(ok ? "" : `${owner}: ${key}`).toBe("");
      }
    }
  });

  test("no two dictionaries define the same key", () => {
    const seen = new Map<string, string>();
    for (const [owner, dict] of dicts) {
      for (const key of Object.keys(dict)) {
        expect(seen.has(key) ? `${key} in ${owner} and ${seen.get(key)}` : "").toBe("");
        seen.set(key, owner);
      }
    }
  });

  test("no text is empty", () => {
    for (const dict of dicts.values()) for (const [k, v] of Object.entries(dict)) expect(v.trim() === "" ? k : "").toBe("");
  });

  test("every t() key used in the code exists in a dictionary", () => {
    const missing: string[] = [];
    let calls = 0;
    for (const file of sources()) {
      const src = readFileSync(file, "utf8");
      for (const m of src.matchAll(/(?<![\w.$])t\(\s*(["'`])([^"'`]*)\1/g)) {
        calls++;
        if (!all.has(m[2]!)) missing.push(`${file.replace(root, "")}: t("${m[2]}")`);
      }
      // a t() call must pass its key as one literal: a built key cannot be checked
      for (const m of src.matchAll(/(?<![\w.$])t\(\s*(`[^`]*\$\{|[^"'`\s)])/g)) missing.push(`${file.replace(root, "")}: t(${m[1]}...) is not a plain string key`);
    }
    expect(calls).toBeGreaterThan(50);
    expect(missing).toEqual([]);
  });

  test("every string that looks like a dictionary key exists (covers keys kept in tables)", () => {
    const missing: string[] = [];
    for (const file of sources()) {
      for (const m of readFileSync(file, "utf8").matchAll(KEY_LITERAL)) {
        if (!all.has(m[1]!)) missing.push(`${file.replace(root, "")}: "${m[1]}"`);
      }
    }
    expect(missing).toEqual([]);
  });

  test("every shell and gallery key is used somewhere (no dead strings)", () => {
    const text = sources().map((f) => readFileSync(f, "utf8")).join("\n");
    expect(Object.keys(shellDict).filter((k) => !text.includes(`"${k}"`))).toEqual([]);
  });
});
