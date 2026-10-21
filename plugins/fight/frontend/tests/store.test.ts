import { afterEach, describe, expect, test } from "bun:test";
import { onPageEvent } from "@arena/shell";
import { addPending, clearPending, getFightStore, removePending, resetFightStore, setLastFight, setPending } from "../src/store";

afterEach(() => resetFightStore());

describe("pending fight store", () => {
  test("adding announces fighter_added once per new KOL, up to 4", () => {
    const heard: string[] = [];
    const off = onPageEvent("fighter_added", (e) => heard.push(e.kol));
    expect(addPending("decu")).toBe(true);
    expect(addPending("decu")).toBe(false);
    for (const k of ["cooker", "tekkerrss", "ansem", "fifth"]) addPending(k);
    off();
    expect(getFightStore().pending).toEqual(["decu", "cooker", "tekkerrss", "ansem"]);
    expect(heard).toEqual(["decu", "cooker", "tekkerrss", "ansem"]);
  });

  test("remove, replace and clear", () => {
    setPending(["a", "b", "a", "c"]);
    expect(getFightStore().pending).toEqual(["a", "b", "c"]);
    removePending("b");
    expect(getFightStore().pending).toEqual(["a", "c"]);
    const heard: string[] = [];
    const off = onPageEvent("fighter_added", (e) => heard.push(e.kol));
    setPending(["c", "d"]);
    off();
    expect(heard).toEqual(["d"]);
    clearPending();
    expect(getFightStore().pending).toEqual([]);
  });

  test("the last fight is kept for Rematch", () => {
    expect(getFightStore().last).toBeNull();
    setLastFight({ ids: ["decu", "cooker"], date: "2026-10-18" });
    expect(getFightStore().last).toEqual({ ids: ["decu", "cooker"], date: "2026-10-18" });
  });
});
