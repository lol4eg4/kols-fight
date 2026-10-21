/**
 * Pixel avatar sprite: a 5x5 grid, mirrored left to right, one palette colour.
 * Pattern and colour come only from the KOL id, so one KOL always looks the same.
 */

export const AVATAR_COLORS = ["coin", "pink", "blue", "lavender", "plus"] as const;
export type AvatarColor = (typeof AVATAR_COLORS)[number];

export interface AvatarSpec {
  /** cells[row][col], 5 rows of 5. Symmetric: cells[r][c] === cells[r][4 - c]. */
  cells: boolean[][];
  color: AvatarColor;
}

/** FNV-1a, 32 bit. */
export function hashId(id: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < id.length; i++) {
    h ^= id.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** mulberry32: small seeded generator. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function build(seed: number): boolean[][] {
  const next = rng(seed);
  const cells: boolean[][] = [];
  for (let r = 0; r < 5; r++) {
    const row = [false, false, false, false, false];
    for (let c = 0; c < 3; c++) {
      const on = next() < 0.55;
      row[c] = on;
      row[4 - c] = on;
    }
    cells.push(row);
  }
  return cells;
}

const count = (cells: boolean[][]) => cells.reduce((n, row) => n + row.filter(Boolean).length, 0);
const everyRowUsed = (cells: boolean[][]) => cells.every((row) => row.some(Boolean));

/** Pure and deterministic: the same id gives the same sprite in every run. */
export function avatarSpec(id: string): AvatarSpec {
  const seed = hashId(id);
  let cells = build(seed);
  // Reject sprites that are nearly empty or full, or that skip a whole row: re-roll with a fixed salt.
  for (let salt = 1; salt <= 32 && !(count(cells) >= 9 && count(cells) <= 17 && everyRowUsed(cells)); salt++) {
    cells = build(Math.imul(seed ^ salt, 0x9e3779b1) >>> 0);
  }
  return { cells, color: AVATAR_COLORS[(seed >>> 11) % AVATAR_COLORS.length]! };
}

/** SVG path of all filled cells as unit squares, offset by (1, 1) inside a 7x7 tile. */
export function avatarPath(cells: boolean[][]): string {
  let d = "";
  cells.forEach((row, r) =>
    row.forEach((on, c) => {
      if (on) d += `M${c + 1} ${r + 1}h1v1h-1z`;
    }),
  );
  return d;
}
