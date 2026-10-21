import { describe, expect, test } from "bun:test";
import { STORAGE_KEY, MAX_PICKS, choose, fightKey, load, save, settle } from "../src/storage";
import type { Saved } from "../src/storage";

const empty = (): Saved => ({ called: 0, played: 0, picks: {} });

/** A Storage that keeps strings in memory. */
function memory(initial: Record<string, string> = {}): Storage {
  const m = new Map(Object.entries(initial));
  return {
    get length() {
      return m.size;
    },
    clear: () => m.clear(),
    getItem: (k) => m.get(k) ?? null,
    key: (i) => [...m.keys()][i] ?? null,
    removeItem: (k) => void m.delete(k),
    setItem: (k, v) => void m.set(k, v),
  };
}

/** A Storage whose every call throws, like a blocked browser. */
function blocked(): Storage {
  const boom = () => {
    throw new DOMException("denied", "SecurityError");
  };
  return { get length() { return boom(); }, clear: boom, getItem: boom, key: boom, removeItem: boom, setItem: boom } as unknown as Storage;
}

describe("storage wrapper", () => {
  test("no storage at all: load is null, save is false, nothing throws", () => {
    expect(load(null)).toBeNull();
    expect(save(empty(), null)).toBe(false);
  });

  test("a storage that throws on every call fails safely", () => {
    expect(load(blocked())).toBeNull();
    expect(save(empty(), blocked())).toBe(false);
  });

  test("a storage whose write fails still reads", () => {
    const s = memory();
    s.setItem = () => {
      throw new DOMException("full", "QuotaExceededError");
    };
    expect(load(s)).toEqual(empty());
    expect(save(empty(), s)).toBe(false);
  });

  test("the global accessor itself may throw", () => {
    const desc = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
    Object.defineProperty(globalThis, "localStorage", {
      configurable: true,
      get() {
        throw new DOMException("denied", "SecurityError");
      },
    });
    try {
      expect(load()).toBeNull();
      expect(save(empty())).toBe(false);
    } finally {
      if (desc) Object.defineProperty(globalThis, "localStorage", desc);
      else delete (globalThis as { localStorage?: unknown }).localStorage;
    }
  });

  test("garbage in storage reads as empty data", () => {
    for (const raw of ["not json", "null", "[]", "42", '{"played":"x","picks":7}', '{"picks":{"a":5,"b":{"kol":3}}}']) {
      expect(load(memory({ [STORAGE_KEY]: raw }))).toEqual(empty());
    }
  });

  test("saved data round-trips", () => {
    const s = memory();
    const key = fightKey(["b", "a"], "2026-10-18");
    const a = settle(choose(empty(), key, "a"), key, "a").saved;
    expect(save(a, s)).toBe(true);
    expect(load(s)).toEqual(a);
  });

  test("only the newest picks are kept", () => {
    let saved = empty();
    for (let i = 0; i < MAX_PICKS + 5; i++) saved = choose(saved, `k${i}`, "a");
    const s = memory();
    save(saved, s);
    const back = load(s)!;
    expect(Object.keys(back.picks)).toHaveLength(MAX_PICKS);
    expect(back.picks.k0).toBeUndefined();
    expect(back.picks[`k${MAX_PICKS + 4}`]).toBeDefined();
  });
});

describe("score", () => {
  const key = fightKey(["a", "b"], "2026-10-18");

  test("the key ignores the order of the ids", () => {
    expect(fightKey(["b", "a"], "2026-10-18")).toBe(key);
    expect(fightKey(["a", "b"], "2026-10-19")).not.toBe(key);
  });

  test("right and wrong guesses", () => {
    const right = settle(choose(empty(), key, "a"), key, "a");
    expect(right.verdict).toBe("called");
    expect([right.saved.called, right.saved.played]).toEqual([1, 1]);
    const wrong = settle(choose(right.saved, "other", "b"), "other", "a");
    expect(wrong.verdict).toBe("missed");
    expect([wrong.saved.called, wrong.saved.played]).toEqual([1, 2]);
  });

  test("a draw and a fight without a guess leave the score alone", () => {
    const draw = settle(choose(empty(), key, "a"), key, null);
    expect(draw.verdict).toBe("draw");
    expect([draw.saved.called, draw.saved.played]).toEqual([0, 0]);
    const none = settle(empty(), key, "a");
    expect(none.verdict).toBeNull();
    expect(none.saved).toEqual(empty());
  });

  test("a fight counts once and its pick is locked", () => {
    const first = settle(choose(empty(), key, "a"), key, "a").saved;
    const again = settle(first, key, "b");
    expect(again.verdict).toBe("called");
    expect(again.saved).toEqual(first);
    expect(choose(first, key, "b")).toEqual(first);
  });

  test("the pick can change before the fight ends", () => {
    const s = choose(choose(empty(), key, "a"), key, "b");
    expect(s.picks[key]).toEqual({ kol: "b", verdict: null });
  });
});
