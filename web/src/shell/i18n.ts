// Dictionary: every UI string lives in a dictionary (shell en.ts, or the plugin's own en.ts) and is read with t().
// Keys: "shell.*" and "gallery.*" belong to the shell; a plugin's keys start with "<plugin>.".

const entries = new Map<string, string>();
const warned = new Set<string>();

/** Adds strings. A key that is already taken by another owner is an error: keys never collide silently. */
export function addDictionary(owner: string, dict: Record<string, string>): void {
  for (const [key, value] of Object.entries(dict)) {
    if (entries.has(key) && entries.get(key) !== value) {
      throw new Error(`dictionary key "${key}" of ${owner} is already defined with another text`);
    }
    entries.set(key, value);
  }
}

export function hasKey(key: string): boolean {
  return entries.has(key);
}

/** The text for a key. `{name}` in the text is replaced from vars. A missing key shows the key itself. */
export function t(key: string, vars?: Record<string, string | number>): string {
  const text = entries.get(key);
  if (text === undefined) {
    if (!warned.has(key)) {
      warned.add(key);
      console.warn(`missing dictionary key "${key}"`);
    }
    return key;
  }
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}
