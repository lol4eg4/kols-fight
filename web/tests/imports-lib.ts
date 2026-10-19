import { readdirSync, statSync } from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";

/** Every file under dir whose name ends with one of the suffixes, skipping node_modules. */
export function walk(dir: string, suffixes: string[]): string[] {
  const out: string[] = [];
  for (const name of readdirSync(dir)) {
    if (name === "node_modules") continue;
    const p = resolve(dir, name);
    if (statSync(p).isDirectory()) out.push(...walk(p, suffixes));
    else if (suffixes.some((s) => name.endsWith(s))) out.push(p);
  }
  return out.sort();
}

function stripComments(src: string): string {
  return src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:"'`\\])\/\/.*$/gm, "$1");
}

/** Module specifiers a TS, TSX or CSS source pulls in: import/export from, import(), require(), CSS @import and url(). */
export function importsOf(source: string, kind: "script" | "css" = "script"): string[] {
  const src = stripComments(source);
  const found: string[] = [];
  if (kind === "css") {
    for (const m of src.matchAll(/@import\s+(?:url\()?\s*["']([^"']+)["']/g)) found.push(m[1]!);
    for (const m of src.matchAll(/url\(\s*["']?([^"')]+)["']?\s*\)/g)) found.push(m[1]!);
    return found;
  }
  for (const m of src.matchAll(/\b(?:import|export)\s+(?:type\s+)?(?:[\w*{}\s,$]+?\s+from\s+)?["']([^"']+)["']/g)) found.push(m[1]!);
  for (const m of src.matchAll(/\bimport\(\s*["']([^"']+)["']\s*\)/g)) found.push(m[1]!);
  for (const m of src.matchAll(/\brequire\(\s*["']([^"']+)["']\s*\)/g)) found.push(m[1]!);
  return found;
}

export interface Rule {
  /** Folder the file belongs to: relative imports must stay inside it. */
  home: string;
  /** Bare specifiers that are allowed. */
  allowed: string[];
}

/** Violations of an import rule in one file. Empty means it follows the rule. */
export function violations(file: string, specifiers: string[], rule: Rule): string[] {
  const out: string[] = [];
  for (const spec of specifiers) {
    if (spec.startsWith(".")) {
      const target = resolve(dirname(file), spec);
      const rel = relative(rule.home, target);
      if (rel.startsWith("..") || rel.split(sep)[0] === "..") out.push(`${spec} leaves ${rule.home}`);
    } else if (!rule.allowed.includes(spec)) {
      out.push(spec.startsWith("@arena/") ? `${spec}: another package` : `${spec}: not allowed`);
    }
  }
  return out;
}
