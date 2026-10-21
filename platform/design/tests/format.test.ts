import { describe, expect, test } from "bun:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { formatMinutesAgo, formatMoneyFull, formatMoneyShort, formatSnapshotDate, formatTimeAgo, moneyParts } from "../src/format";
import { Money } from "../src/components/Money";
import { SnapshotDate } from "../src/components/SnapshotDate";
import { TimeAgo } from "../src/components/TimeAgo";

const M = "−"; // true minus

describe("Money, short (tables)", () => {
  test("examples from the Brief and the prototype", () => {
    expect(formatMoneyShort(207060)).toBe("+$207.1K");
    expect(formatMoneyShort(-1600)).toBe(`${M}$1.6K`);
    expect(formatMoneyShort(5300000)).toBe("+$5.3M");
    expect(formatMoneyShort(24400)).toBe("+$24.4K");
    expect(formatMoneyShort(565100)).toBe("+$565.1K");
  });
  test("a .0 is dropped", () => {
    expect(formatMoneyShort(17000)).toBe("+$17K");
    expect(formatMoneyShort(4000)).toBe("+$4K");
    expect(formatMoneyShort(2000000)).toBe("+$2M");
  });
  test("below 1000 whole dollars", () => {
    expect(formatMoneyShort(629)).toBe("+$629");
    expect(formatMoneyShort(-629)).toBe(`${M}$629`);
    expect(formatMoneyShort(339.4)).toBe("+$339");
  });
  test("rolls over at the unit edge", () => {
    expect(formatMoneyShort(999.4)).toBe("+$999");
    expect(formatMoneyShort(999.6)).toBe("+$1K");
    expect(formatMoneyShort(999949)).toBe("+$999.9K");
    expect(formatMoneyShort(999950)).toBe("+$1M");
    expect(formatMoneyShort(1.25e9)).toBe("+$1.3B");
  });
  test("zero has no sign; so does a value that rounds to zero", () => {
    expect(formatMoneyShort(0)).toBe("$0");
    expect(formatMoneyShort(0.3)).toBe("$0");
    expect(formatMoneyShort(-0.3)).toBe("$0");
    expect(moneyParts(0).sign).toBe("zero");
  });
});

describe("Money, full (card)", () => {
  test("grouped, signed", () => {
    expect(formatMoneyFull(207060)).toBe("+$207,060");
    expect(formatMoneyFull(-298530)).toBe(`${M}$298,530`);
    expect(formatMoneyFull(1280000)).toBe("+$1,280,000");
    expect(formatMoneyFull(1530)).toBe("+$1,530");
    expect(formatMoneyFull(0)).toBe("$0");
  });
  test("an amount that is not a result has no plus", () => {
    expect(moneyParts(1280000, "full", { signed: false }).text).toBe("$1,280,000");
    expect(moneyParts(1500, "short", { signed: false }).text).toBe("$1.5K");
    expect(moneyParts(1500, "short", { signed: false }).sign).toBe("zero");
  });
  test("not finite is refused", () => {
    expect(() => moneyParts(Number.NaN)).toThrow(RangeError);
    expect(() => moneyParts(Infinity)).toThrow(RangeError);
  });
});

describe("Money component", () => {
  test("green plus, red minus, sign always in the text", () => {
    const plus = renderToStaticMarkup(createElement(Money, { value: 207060 }));
    expect(plus).toContain("ds-money--plus");
    expect(plus).toContain("+$207.1K");
    const minus = renderToStaticMarkup(createElement(Money, { value: -1600 }));
    expect(minus).toContain("ds-money--minus");
    expect(minus).toContain(`${M}$1.6K`);
  });
  test("no data is a dash with its reason, never a zero", () => {
    const html = renderToStaticMarkup(createElement(Money, { value: null, reason: "No 24h PnL here" }));
    expect(html).toContain("—");
    expect(html).toContain("No 24h PnL here");
    expect(html).not.toContain("$0");
    const text = renderToStaticMarkup(createElement(Money, { value: undefined, reason: "Why", reasonMode: "text" }));
    expect(text).toContain("ds-nodata__reason");
  });
});

describe("TimeAgo", () => {
  test("minutes, hours, days", () => {
    expect(formatMinutesAgo(0)).toBe("just now");
    expect(formatMinutesAgo(0.9)).toBe("just now");
    expect(formatMinutesAgo(1)).toBe("1 min ago");
    expect(formatMinutesAgo(59)).toBe("59 min ago");
    expect(formatMinutesAgo(60)).toBe("1h ago");
    expect(formatMinutesAgo(120)).toBe("2h ago");
    expect(formatMinutesAgo(23 * 60 + 59)).toBe("23h ago");
    expect(formatMinutesAgo(24 * 60)).toBe("1d ago");
    expect(formatMinutesAgo(5 * 24 * 60)).toBe("5d ago");
    expect(formatMinutesAgo(31 * 24 * 60)).toBe("31d ago");
  });
  test("a time in the future counts as just now", () => {
    expect(formatMinutesAgo(-5)).toBe("just now");
  });
  test("from two points in time", () => {
    expect(formatTimeAgo("2026-10-18T04:00:00Z", "2026-10-18T06:00:00Z")).toBe("2h ago");
    expect(formatTimeAgo(Date.UTC(2026, 9, 13, 6), Date.UTC(2026, 9, 18, 6, 30))).toBe("5d ago");
  });
  test("component: minutes, at, and no data", () => {
    expect(renderToStaticMarkup(createElement(TimeAgo, { minutes: 120 }))).toContain("2h ago");
    const at = renderToStaticMarkup(createElement(TimeAgo, { at: "2026-10-18T04:00:00Z", now: "2026-10-18T06:00:00Z" }));
    expect(at).toContain("2h ago");
    expect(at).toMatch(/datetime="2026-10-18T04:00:00.000Z"/i);
    const none = renderToStaticMarkup(createElement(TimeAgo, { minutes: null, reason: "No trades yet" }));
    expect(none).toContain("—");
    expect(none).toContain("No trades yet");
  });
});

describe("SnapshotDate", () => {
  test("UTC, as in the Brief", () => {
    expect(formatSnapshotDate("2026-10-18T06:00:00Z")).toBe("Oct 18, 2026, 06:00 UTC");
    expect(formatSnapshotDate(Date.UTC(2026, 0, 5, 14, 7))).toBe("Jan 5, 2026, 14:07 UTC");
    expect(formatSnapshotDate(new Date("2026-12-31T23:59:00Z"))).toBe("Dec 31, 2026, 23:59 UTC");
  });
  test("an offset time is shown in UTC", () => {
    expect(formatSnapshotDate("2026-10-18T09:00:00+03:00")).toBe("Oct 18, 2026, 06:00 UTC");
    expect(formatSnapshotDate("2026-10-18T01:30:00-05:00")).toBe("Oct 18, 2026, 06:30 UTC");
  });
  test("without the year, and a plain day", () => {
    expect(formatSnapshotDate("2026-10-18T06:00:00Z", { year: false })).toBe("Oct 18, 06:00 UTC");
    expect(formatSnapshotDate("2026-10-18")).toBe("Oct 18, 2026");
    expect(formatSnapshotDate("2026-10-08", { year: false })).toBe("Oct 8");
  });
  test("a bad date is refused", () => {
    expect(() => formatSnapshotDate("not a date")).toThrow(RangeError);
    expect(() => formatSnapshotDate("2026-13-40")).toThrow(RangeError);
  });
  test("component", () => {
    const html = renderToStaticMarkup(createElement(SnapshotDate, { at: "2026-10-18T06:00:00Z" }));
    expect(html).toContain("Oct 18, 2026, 06:00 UTC");
    expect(html).toMatch(/datetime="2026-10-18T06:00:00.000Z"/i);
    expect(renderToStaticMarkup(createElement(SnapshotDate, { at: null, reason: "No snapshot" }))).toContain("No snapshot");
  });
});
