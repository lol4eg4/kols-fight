import { describe, expect, test } from "bun:test";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AVATAR_COLORS, avatarSpec, hashId } from "../src/avatar";
import { PixelAvatar } from "../src/components/PixelAvatar";

const IDS = ["tekkerrss", "levis", "decu", "cooker", "trunoest", "peter", "kook", "mrpunk-eth", "ocr", "laseasy10", "naka", "babatooonde", "shah", "iced", "juicy"];

describe("PixelAvatar", () => {
  test("the same id gives the same sprite and the same markup, every time", () => {
    for (const id of IDS) {
      expect(avatarSpec(id)).toEqual(avatarSpec(id));
      const a = renderToStaticMarkup(createElement(PixelAvatar, { id }));
      const b = renderToStaticMarkup(createElement(PixelAvatar, { id }));
      expect(a).toBe(b);
    }
  });

  test("the hash is fixed (a change would repaint every KOL)", () => {
    expect(hashId("tekkerrss")).toBe(hashId("tekkerrss"));
    expect(hashId("")).toBe(0x811c9dc5);
    expect(hashId("a")).toBe(0xe40c292c); // FNV-1a 32 of "a"
    const { cells, color } = avatarSpec("tekkerrss");
    expect(cells.map((row) => row.map((on) => (on ? "#" : ".")).join(""))).toEqual(["#####", ".#.#.", "#...#", "#####", ".###."]);
    expect(color).toBe("coin");
  });

  test("5x5, mirrored left to right, one palette colour", () => {
    for (const id of IDS) {
      const { cells, color } = avatarSpec(id);
      expect(cells).toHaveLength(5);
      for (const row of cells) {
        expect(row).toHaveLength(5);
        expect(row[0]).toBe(row[4]);
        expect(row[1]).toBe(row[3]);
      }
      expect(AVATAR_COLORS).toContain(color);
    }
  });

  test("never empty or full, and no empty row", () => {
    for (let i = 0; i < 500; i++) {
      const { cells } = avatarSpec(`kol-${i}`);
      const on = cells.flat().filter(Boolean).length;
      expect(on).toBeGreaterThanOrEqual(9);
      expect(on).toBeLessThanOrEqual(17);
      expect(cells.every((row) => row.some(Boolean))).toBe(true);
    }
  });

  test("different ids give different sprites (most of the time)", () => {
    const seen = new Set(IDS.map((id) => JSON.stringify(avatarSpec(id))));
    expect(seen.size).toBeGreaterThanOrEqual(IDS.length - 1);
    const colors = new Set(IDS.map((id) => avatarSpec(id).color));
    expect(colors.size).toBeGreaterThanOrEqual(3);
  });

  test("decorative: hidden from screen readers, no alt text, no outside image", () => {
    const html = renderToStaticMarkup(createElement(PixelAvatar, { id: "decu", size: 48 }));
    expect(html).toContain('aria-hidden="true"');
    expect(html).toContain('width="48"');
    expect(html).not.toMatch(/<img|href=|src=|aria-label|<title/);
  });
});
