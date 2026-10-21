import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { importsOf, violations, walk } from "./imports-lib";

const root = resolve(import.meta.dir, "../..");
const pluginNames = readdirSync(resolve(root, "plugins")).filter((n) => existsSync(resolve(root, "plugins", n, "plugin.toml")));

// A plugin frontend imports only react, @arena/design, @arena/shell and its own files.
const PLUGIN_ALLOWED = ["react", "react/jsx-runtime", "react/jsx-dev-runtime", "@arena/design", "@arena/shell"];
// @arena/design imports no plugin and no shell; React and React DOM are its only dependencies.
const DESIGN_ALLOWED = ["react", "react/jsx-runtime", "react/jsx-dev-runtime", "react-dom"];

describe("import rule: plugin frontends", () => {
  test("there is a frontend for every plugin", () => {
    expect(pluginNames).toHaveLength(21);
    for (const n of pluginNames) expect(existsSync(resolve(root, "plugins", n, "frontend", "src", "index.ts"))).toBe(true);
  });

  for (const name of pluginNames) {
    test(`${name}: source imports only react, @arena/design, @arena/shell and its own files`, () => {
      const home = resolve(root, "plugins", name, "frontend");
      const bad: string[] = [];
      for (const file of walk(resolve(home, "src"), [".ts", ".tsx", ".css"])) {
        const kind = file.endsWith(".css") ? "css" : "script";
        for (const v of violations(file, importsOf(readFileSync(file, "utf8"), kind), { home, allowed: PLUGIN_ALLOWED })) bad.push(`${file}: ${v}`);
      }
      expect(bad).toEqual([]);
    });

    test(`${name}: package.json names @arena/${name} and depends on nothing else`, () => {
      const pkg = JSON.parse(readFileSync(resolve(root, "plugins", name, "frontend", "package.json"), "utf8"));
      expect(pkg.name).toBe(`@arena/${name}`);
      expect(Object.keys(pkg.dependencies ?? {}).sort()).toEqual(["@arena/design", "@arena/shell", "react"]);
      expect(pkg.devDependencies).toBeUndefined();
    });
  }
});

describe("import rule: @arena/design", () => {
  const home = resolve(root, "platform/design");
  const files = walk(resolve(home, "src"), [".ts", ".tsx", ".css"]);

  test("it has source files", () => {
    expect(files.length).toBeGreaterThan(20);
  });

  test("imports no plugin and no shell, only react and its own files", () => {
    const bad: string[] = [];
    for (const file of files) {
      const kind = file.endsWith(".css") ? "css" : "script";
      const specs = importsOf(readFileSync(file, "utf8"), kind).filter(
        // font files come from the @fontsource packages through url() in fonts.css
        (s) => !(kind === "css" && /^@fontsource\/[\w-]+\/files\/[\w.-]+\.woff2$/.test(s)),
      );
      for (const v of violations(file, specs, { home, allowed: DESIGN_ALLOWED })) bad.push(`${file}: ${v}`);
    }
    expect(bad).toEqual([]);
  });

  test("calls no route: no fetch, no XHR, no socket, no /api path", () => {
    const bad: string[] = [];
    for (const file of files.filter((f) => !f.endsWith(".css"))) {
      const src = readFileSync(file, "utf8").replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
      for (const re of [/\bfetch\s*\(/, /XMLHttpRequest/, /\bWebSocket\b/, /\bEventSource\b/, /sendBeacon/, /["'`]\/api\//]) {
        if (re.test(src)) bad.push(`${file}: ${re}`);
      }
    }
    expect(bad).toEqual([]);
  });

  test("package.json depends on react, react-dom and the three font packages only", () => {
    const pkg = JSON.parse(readFileSync(resolve(home, "package.json"), "utf8"));
    expect(pkg.name).toBe("@arena/design");
    expect(Object.keys(pkg.dependencies).sort()).toEqual(["@fontsource/golos-text", "@fontsource/jetbrains-mono", "@fontsource/press-start-2p", "react", "react-dom"]);
  });
});

describe("the import checker itself", () => {
  const home = "/repo/plugins/x/frontend";
  const rule = { home, allowed: PLUGIN_ALLOWED };
  const file = `${home}/src/index.ts`;

  test("finds every import form", () => {
    const src = `
      import a from "react";
      import type { B } from '@arena/design';
      import "./side.css";
      export * from "./all";
      export { c } from "../other";
      const d = await import("@arena/board");
      const e = require("lodash");
      // import ignored from "comment";
      /* import ignored2 from "block"; */
    `;
    expect(importsOf(src)).toEqual(["react", "@arena/design", "./side.css", "./all", "../other", "@arena/board", "lodash"]);
  });

  test("flags another plugin, an outside package and a path out of the folder", () => {
    expect(violations(file, ["react", "@arena/design", "@arena/shell", "./en", "../src/en"], rule)).toEqual([]);
    expect(violations(file, ["@arena/board"], rule)).toEqual(["@arena/board: another package"]);
    expect(violations(file, ["lodash", "react-dom"], rule)).toEqual(["lodash: not allowed", "react-dom: not allowed"]);
    expect(violations(file, ["../../../board/frontend/src/x"], rule)).toHaveLength(1);
  });

  test("design may not import the shell", () => {
    const d = { home: "/repo/platform/design", allowed: DESIGN_ALLOWED };
    expect(violations("/repo/platform/design/src/a.ts", ["@arena/shell"], d)).toEqual(["@arena/shell: another package"]);
  });
});
