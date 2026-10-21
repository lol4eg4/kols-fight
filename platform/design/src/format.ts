/**
 * Number and time formatting for the client page. English only for now: every format the Brief fixes
 * lives here, so a later translation changes this one file.
 */

export type MoneySign = "plus" | "minus" | "zero";
export type MoneyMode = "short" | "full";

const MINUS = "−"; // the true minus sign, as in the Brief

function oneDecimal(n: number): string {
  const s = n.toFixed(1);
  return s.endsWith(".0") ? s.slice(0, -2) : s;
}

function group(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/** Short body without sign: 629, 1.5K, 17K, 207.1K, 5.3M. Rolls over at the unit edge: 999.95K is 1M. */
function shortBody(abs: number): string {
  if (abs < 999.5) return String(Math.round(abs));
  const units: Array<[number, string]> = [
    [1e3, "K"],
    [1e6, "M"],
    [1e9, "B"],
  ];
  for (let i = 0; i < units.length; i++) {
    const [div, suffix] = units[i]!;
    const n = Math.round((abs / div) * 10) / 10;
    if (n < 1000 || i === units.length - 1) return `${oneDecimal(n)}${suffix}`;
  }
  return String(Math.round(abs));
}

/**
 * Dollars. A profit or loss (signed, the default) always carries its sign: +$207.1K, −$1.6K, +$5.3M;
 * full mode gives +$207,060. A value that rounds to zero is "$0", with no sign and no colour.
 * An amount that is not a result (position size, volume) passes signed: false and prints $1.5K, no plus.
 */
export function moneyParts(
  value: number,
  mode: MoneyMode = "short",
  { signed = true }: { signed?: boolean } = {},
): { text: string; sign: MoneySign } {
  if (!Number.isFinite(value)) throw new RangeError(`not a finite amount: ${value}`);
  const abs = Math.abs(value);
  const body = mode === "short" ? shortBody(abs) : group(Math.round(abs));
  if (body === "0") return { text: "$0", sign: "zero" };
  if (value < 0) return { text: `${MINUS}$${body}`, sign: signed ? "minus" : "zero" };
  return signed ? { text: `+$${body}`, sign: "plus" } : { text: `$${body}`, sign: "zero" };
}

export const formatMoneyShort = (value: number) => moneyParts(value, "short").text;
export const formatMoneyFull = (value: number) => moneyParts(value, "full").text;

/** Time since an event, from minutes: "59 min ago", "2h ago", "5d ago". */
export function formatMinutesAgo(minutes: number): string {
  if (!Number.isFinite(minutes)) throw new RangeError(`not a finite number of minutes: ${minutes}`);
  const m = Math.max(0, Math.floor(minutes));
  if (m < 1) return "just now";
  if (m < 60) return `${m} min ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

/** Same, from a point in time and "now" (milliseconds, a Date or an ISO string). */
export function formatTimeAgo(at: string | number | Date, now: string | number | Date = Date.now()): string {
  const diff = toDate(now).getTime() - toDate(at).getTime();
  return formatMinutesAgo(diff / 60000);
}

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const pad = (n: number) => String(n).padStart(2, "0");

function toDate(v: string | number | Date): Date {
  const d = v instanceof Date ? v : new Date(v);
  if (Number.isNaN(d.getTime())) throw new RangeError(`not a date: ${String(v)}`);
  return d;
}

export interface SnapshotDateOptions {
  /** Leave the year out ("Oct 18, 06:00 UTC"), for tight places like the stale-data bar. Default true. */
  year?: boolean;
}

/**
 * Snapshot time in UTC: "Oct 18, 2026, 06:00 UTC". A plain day "2026-10-18" gives "Oct 18, 2026".
 * The hand-written format does not depend on the browser's locale data.
 */
export function formatSnapshotDate(input: string | number | Date, { year = true }: SnapshotDateOptions = {}): string {
  if (typeof input === "string") {
    const day = input.match(/^(\d{4})-(\d{2})-(\d{2})$/);
    if (day) {
      const month = MONTHS[Number(day[2]) - 1];
      if (!month) throw new RangeError(`not a date: ${input}`);
      return year ? `${month} ${Number(day[3])}, ${day[1]}` : `${month} ${Number(day[3])}`;
    }
  }
  const d = toDate(input);
  const date = `${MONTHS[d.getUTCMonth()]} ${d.getUTCDate()}${year ? `, ${d.getUTCFullYear()}` : ""}`;
  return `${date}, ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())} UTC`;
}

/** Points of a score part: one decimal, no trailing ".0". */
export function formatPoints(points: number): string {
  return oneDecimal(points);
}
