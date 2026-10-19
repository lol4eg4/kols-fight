import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { designPath, parseFrontMatter, renderTokens, tokensPath } from "../../../scripts/gen-tokens";

const md = readFileSync(designPath, "utf8");
const front = parseFrontMatter(md) as Record<string, Record<string, unknown>>;
const css = readFileSync(tokensPath, "utf8");

// --- read the generated CSS back: custom properties of :root ---
const vars = new Map<string, string>();
for (const m of css.matchAll(/^\s*(--[\w-]+):\s*(.+);$/gm)) vars.set(m[1]!, m[2]!);

/** Follows var(--x) chains to the literal value. */
function resolveVar(name: string): string {
  const v = vars.get(name);
  if (v === undefined) throw new Error(`${name} is not in tokens.css`);
  return v.replace(/var\((--[\w-]+)\)/g, (_, n: string) => resolveVar(n));
}

/** Follows {section.key.prop} references in DESIGN.md to the literal value. */
function resolveRef(value: unknown): string {
  const s = String(value);
  const ref = s.match(/^\{([^}]+)\}$/);
  if (!ref) return s;
  let node: unknown = front;
  for (const part of ref[1]!.split(".")) node = (node as Record<string, unknown>)[part];
  return resolveRef(node);
}

const kebab = (s: string) => s.replace(/[A-Z]/g, (c) => "-" + c.toLowerCase());
const unquote = (s: string) => s.replace(/^"(.*)"$/, "$1");

describe("tokens.css equals the front matter of DESIGN.md", () => {
  test("colors", () => {
    for (const [k, v] of Object.entries(front.colors!)) expect(resolveVar(`--color-${k}`)).toBe(resolveRef(v));
  });
  test("rounded and spacing", () => {
    for (const [k, v] of Object.entries(front.rounded!)) expect(resolveVar(`--radius-${k}`)).toBe(String(v));
    for (const [k, v] of Object.entries(front.spacing!)) expect(resolveVar(`--space-${k}`)).toBe(String(v));
  });
  test("typography", () => {
    for (const [k, props] of Object.entries(front.typography!)) {
      for (const [p, v] of Object.entries(props as Record<string, unknown>)) {
        const prop = p === "fontFeature" ? "font-feature-settings" : kebab(p);
        expect(unquote(resolveVar(`--type-${k}-${prop}`))).toBe(String(v));
      }
    }
  });
  test("components", () => {
    for (const [k, props] of Object.entries(front.components!)) {
      for (const [p, v] of Object.entries(props as Record<string, unknown>)) {
        expect(unquote(resolveVar(`--component-${k}-${kebab(p)}`))).toBe(resolveRef(v));
      }
    }
  });
  test("every custom property in the file comes from DESIGN.md", () => {
    const expected = new Set<string>();
    for (const [section, prefix] of [["colors", "color"], ["rounded", "radius"], ["spacing", "space"]] as const) {
      for (const k of Object.keys(front[section]!)) expected.add(`--${prefix}-${k}`);
    }
    for (const [section, prefix] of [["typography", "type"], ["components", "component"]] as const) {
      for (const [k, props] of Object.entries(front[section]!)) {
        for (const p of Object.keys(props as object)) expected.add(`--${prefix}-${k}-${p === "fontFeature" ? "font-feature-settings" : kebab(p)}`);
      }
    }
    expect([...vars.keys()].sort()).toEqual([...expected].sort());
  });
  test("the committed file is what the generator writes (run: bun scripts/gen-tokens.ts)", () => {
    expect(css).toBe(renderTokens(md));
  });
});

// --- WCAG contrast ---
function lum(hex: string): number {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * c[0]! + 0.7152 * c[1]! + 0.0722 * c[2]!;
}
function contrast(a: string, b: string): number {
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi! + 0.05) / (lo! + 0.05);
}
function mix(fg: string, bg: string, share: number): string {
  const ch = (i: number) => Math.round(parseInt(fg.slice(i, i + 2), 16) * share + parseInt(bg.slice(i, i + 2), 16) * (1 - share));
  return "#" + [1, 3, 5].map((i) => ch(i).toString(16).padStart(2, "0")).join("");
}
const color = (k: string) => String(front.colors![k]).startsWith("{") ? resolveRef(front.colors![k]) : String(front.colors![k]);

describe("contrast of the palette is at least 4.5:1 where text sits", () => {
  const grounds = ["background", "surface", "surface-sunken"];
  test("body and muted text on every ground", () => {
    for (const fg of ["text", "text-muted"]) for (const bg of grounds) expect(contrast(color(fg), color(bg))).toBeGreaterThanOrEqual(4.5);
  });
  test("accent text on every ground (money, tiers, links, star)", () => {
    for (const fg of ["coin", "pink", "blue", "lavender", "plus", "minus"]) {
      for (const bg of grounds) expect(contrast(color(fg), color(bg))).toBeGreaterThanOrEqual(4.5);
    }
  });
  test("dark text on the yellow and pink buttons", () => {
    expect(contrast(color("on-coin"), color("coin"))).toBeGreaterThanOrEqual(4.5);
    expect(contrast(color("on-pink"), color("pink"))).toBeGreaterThanOrEqual(4.5);
  });
  test("tier text on its own 12% tint over a panel", () => {
    for (const tier of ["tier-goat", "tier-diamond", "tier-sendoor", "tier-degen"]) {
      expect(contrast(color(tier), mix(color(tier), color("surface"), 0.12))).toBeGreaterThanOrEqual(4.5);
    }
  });
  test("tooltip: dark text on the light bubble", () => {
    expect(contrast(color("on-coin"), color("text"))).toBeGreaterThanOrEqual(4.5);
  });
});

test("DESIGN.md has no light theme and no color outside the palette in the design CSS", () => {
  const palette = new Set(Object.values(front.colors!).map((v) => String(v).toLowerCase()));
  for (const file of ["base.css", "components.css", "kol.css"]) {
    const src = readFileSync(resolve(import.meta.dir, "../src", file), "utf8");
    const hex = src.match(/#[0-9a-fA-F]{3,8}\b/g) ?? [];
    for (const h of hex) expect(palette.has(h.toLowerCase())).toBe(true);
    expect(src).not.toMatch(/\brgba?\(|\bhsla?\(/); // colours come from tokens (color-mix of tokens is fine)
    expect(src).not.toMatch(/blur\(|backdrop-filter|filter:\s*drop-shadow/); // no blur, glow or glass
  }
});
