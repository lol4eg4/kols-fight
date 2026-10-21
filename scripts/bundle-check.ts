// Checks that the chunks a client page loads (`/`, `/kol/:id`, `/fight`) hold no secret and no
// private sheet cell. Run after `bun run build` in web/:
//   bun scripts/bundle-check.ts --env <site.env> --sheet <sheet.csv>
// Prints only counts and where a match was found, never the matched value.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..");
const DIST = join(ROOT, "web/dist");
const CLIENT_PATHS = ["/", "/kol/:id", "/fight"];
const SECRET_KEYS = ["SOLANA_TRACKER_API_KEY", "NETWORK_SHEET_CSV_URL", "FIRST_ADMIN_PASSWORD"];
const PRIVATE_COLUMNS = ["Group", "Low $", "High $", "Deliverable / note"];

type Chunk = { file: string; src?: string; isEntry?: boolean; imports?: string[]; dynamicImports?: string[]; css?: string[] };
type Manifest = Record<string, Chunk>;

/** Source file of each client screen, from the plugin manifests and each plugin's `screens` loaders. */
export function clientScreenSources(root: string): string[] {
  const out: string[] = [];
  for (const name of readdirSync(join(root, "plugins"))) {
    let toml: string;
    try {
      toml = readFileSync(join(root, "plugins", name, "plugin.toml"), "utf8");
    } catch {
      continue;
    }
    const index = readFileSync(join(root, "plugins", name, "frontend/src/index.ts"), "utf8");
    for (const m of toml.matchAll(/\{\s*name\s*=\s*"([^"]+)"\s*,\s*path\s*=\s*"([^"]+)"/g)) {
      const [, screen, path] = m;
      if (!CLIENT_PATHS.includes(path!)) continue;
      const load = index.match(new RegExp(`\\b${screen}\\s*:\\s*\\(\\)\\s*=>\\s*import\\("\\./([^"]+)"\\)`));
      if (!load) throw new Error(`plugins/${name}: no loader for screen ${screen} (${path})`);
      out.push(`../plugins/${name}/frontend/src/${load[1]}.tsx`);
    }
  }
  return out;
}

/** Output files a client page can load: the entry's static imports, plus each client screen with everything it imports. */
export function clientFiles(manifest: Manifest, screens: string[]): Set<string> {
  const files = new Set<string>();
  const seen = new Set<string>();
  const walk = (key: string, dynamic: boolean) => {
    if (seen.has(key)) return;
    seen.add(key);
    const c = manifest[key];
    if (!c) throw new Error(`not in the Vite manifest: ${key}`);
    files.add(c.file);
    for (const css of c.css ?? []) files.add(css);
    for (const i of c.imports ?? []) walk(i, dynamic);
    if (dynamic) for (const i of c.dynamicImports ?? []) walk(i, dynamic);
  };
  for (const [key, c] of Object.entries(manifest)) if (c.isEntry) walk(key, false);
  for (const s of screens) walk(s, true);
  return files;
}

/** Values of the secret keys in a `KEY=value` env file. */
export function secretsFromEnv(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim().replace(/^export\s+/, ""))
    .filter((l) => l && !l.startsWith("#"))
    .map((l) => [l.slice(0, l.indexOf("=")), l.slice(l.indexOf("=") + 1).replace(/^(["'])(.*)\1$/, "$2")] as const)
    .filter(([k, v]) => SECRET_KEYS.includes(k) && v.length > 0)
    .map(([, v]) => v);
}

export function parseCsv(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quoted) {
      if (ch === '"' && text[i + 1] === '"') (cell += '"'), i++;
      else if (ch === '"') quoted = false;
      else cell += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ",") row.push(cell), (cell = "");
    else if (ch === "\n" || ch === "\r") {
      if (ch === "\r" && text[i + 1] === "\n") i++;
      row.push(cell), rows.push(row), (row = []), (cell = "");
    } else cell += ch;
  }
  if (cell || row.length) row.push(cell), rows.push(row);
  return rows;
}

/** Private cells of the network sheet: rates, groups, notes, and Wallet cells that are not a Solana address. */
export function privateCells(csv: string): string[] {
  const [header, ...rows] = parseCsv(csv);
  if (!header) return [];
  const cols = PRIVATE_COLUMNS.map((c) => header.indexOf(c)).filter((i) => i >= 0);
  const wallet = header.indexOf("Wallet");
  const cells = new Set<string>();
  for (const r of rows) {
    for (const i of cols) cells.add((r[i] ?? "").trim());
    const w = (r[wallet] ?? "").trim();
    if (wallet >= 0 && !/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(w)) cells.add(w);
  }
  // Short cells (a bare number, "-") match minified code by chance; they say nothing private.
  return [...cells].filter((c) => c.length >= 5);
}

function arg(name: string): string {
  const i = process.argv.indexOf(name);
  const v = i >= 0 ? process.argv[i + 1] : undefined;
  if (!v) throw new Error(`usage: bun scripts/bundle-check.ts --env <site.env> --sheet <sheet.csv> (missing ${name})`);
  return v;
}

if (import.meta.main) {
  const manifest: Manifest = JSON.parse(readFileSync(join(DIST, ".vite/manifest.json"), "utf8"));
  const screens = clientScreenSources(ROOT);
  if (screens.length < CLIENT_PATHS.length) throw new Error(`found ${screens.length} client screens, want ${CLIENT_PATHS.length}`);
  const files = clientFiles(manifest, screens);
  const secrets = secretsFromEnv(readFileSync(arg("--env"), "utf8"));
  if (secrets.length < SECRET_KEYS.length) throw new Error(`env file holds ${secrets.length} of ${SECRET_KEYS.length} secrets`);
  const cells = privateCells(readFileSync(arg("--sheet"), "utf8"));
  if (cells.length === 0) throw new Error("sheet has no private cells: wrong file?");
  let found = 0;
  for (const f of files) {
    const text = readFileSync(join(DIST, f), "utf8");
    secrets.forEach((s, i) => text.includes(s) && (found++, console.log(`LEAK ${f}: secret #${i + 1}`)));
    cells.forEach((c, i) => text.includes(c) && (found++, console.log(`LEAK ${f}: sheet cell #${i + 1}`)));
  }
  console.log(`${files.size} client files, ${secrets.length} secrets, ${cells.length} sheet cells checked: ${found} leaks`);
  process.exit(found ? 1 : 0);
}
