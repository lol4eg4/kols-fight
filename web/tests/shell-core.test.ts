import { afterEach, beforeEach, describe, expect, spyOn, test } from "bun:test";
import { api, emitPageEvent, onPageEvent, t } from "../src/shell";
import { addDictionary } from "../src/shell/i18n";
import { getSessionState, loadSession, resetSession, signOut } from "../src/shell/session";

describe("t()", () => {
  addDictionary("test", { "test.hello": "Hello, {name}!", "test.plain": "Plain", "test.two": "{a} of {b}" });

  test("plain, with variables, and unknown variables stay", () => {
    expect(t("test.plain")).toBe("Plain");
    expect(t("test.hello", { name: "Decu" })).toBe("Hello, Decu!");
    expect(t("test.two", { a: 3, b: 5 })).toBe("3 of 5");
    expect(t("test.hello")).toBe("Hello, {name}!");
  });

  test("a missing key shows the key and warns once", () => {
    const warn = spyOn(console, "warn").mockImplementation(() => {});
    expect(t("test.missing")).toBe("test.missing");
    expect(t("test.missing")).toBe("test.missing");
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });

  test("the same key with another text is refused", () => {
    expect(() => addDictionary("other", { "test.plain": "Different" })).toThrow(/already defined/);
    expect(() => addDictionary("other", { "test.plain": "Plain" })).not.toThrow();
  });
});

describe("page events", () => {
  test("listeners get the payload in subscription order and can unsubscribe", () => {
    const seen: string[] = [];
    const off1 = onPageEvent("fighter_added", (p) => seen.push(`a:${p.kol}`));
    const off2 = onPageEvent("fighter_added", (p) => seen.push(`b:${p.kol}`));
    emitPageEvent("fighter_added", { kol: "decu" });
    off1();
    emitPageEvent("fighter_added", { kol: "levis" });
    off2();
    emitPageEvent("fighter_added", { kol: "ignored" });
    expect(seen).toEqual(["a:decu", "b:decu", "b:levis"]);
  });

  test("a listener that throws does not stop the others or the emitter", () => {
    const err = spyOn(console, "error").mockImplementation(() => {});
    const seen: Array<string | null> = [];
    const off1 = onPageEvent("fight_over", () => {
      throw new Error("boom");
    });
    const off2 = onPageEvent("fight_over", (p) => seen.push(p.winner));
    expect(() => emitPageEvent("fight_over", { ids: ["a", "b"], date: "2026-10-18", winner: null })).not.toThrow();
    expect(seen).toEqual([null]);
    expect(err).toHaveBeenCalled();
    off1();
    off2();
    err.mockRestore();
  });
});

describe("api()", () => {
  const realFetch = globalThis.fetch;
  let calls: Array<{ url: string; init: RequestInit | undefined }> = [];
  const reply = (status: number, body: string, type = "application/json") => {
    globalThis.fetch = (async (url: string, init?: RequestInit) => {
      calls.push({ url: String(url), init });
      return new Response(body, { status, headers: { "content-type": type } });
    }) as typeof fetch;
  };
  beforeEach(() => {
    calls = [];
  });
  afterEach(() => {
    globalThis.fetch = realFetch;
  });

  test("returns {data} for a 2xx JSON answer", async () => {
    reply(200, '{"rows":[1,2]}');
    const r = await api<{ rows: number[] }>("/api/board");
    expect(r).toEqual({ data: { rows: [1, 2] } });
    expect(calls[0]!.init?.credentials).toBe("same-origin");
  });

  test("an empty body is {data: undefined}", async () => {
    reply(204, "");
    const r = await api("/api/auth/logout", { method: "POST" });
    expect(r.error).toBeUndefined();
    expect("data" in r).toBe(true);
  });

  test("sends a JSON body as POST with a content type", async () => {
    reply(200, "{}");
    await api("/api/auth/login", { body: { login: "a", password: "b" } });
    expect(calls[0]!.init?.method).toBe("POST");
    expect(calls[0]!.init?.body).toBe('{"login":"a","password":"b"}');
    expect((calls[0]!.init?.headers as Record<string, string>)["Content-Type"]).toBe("application/json");
  });

  test("a refusal is {error} with the server's message and code", async () => {
    reply(429, '{"error":{"code":"locked","message":"Too many tries. Try again in 15 minutes."}}');
    const r = await api("/api/auth/login", { method: "POST", body: {} });
    expect(r).toEqual({ error: { status: 429, message: "Too many tries. Try again in 15 minutes.", code: "locked" } });
  });

  test("a plain-string error body and a bare message both work", async () => {
    reply(400, '{"error":"Wrong login or password"}');
    expect((await api("/x")).error?.message).toBe("Wrong login or password");
    reply(400, '{"message":"Bad date"}');
    expect((await api("/x")).error?.message).toBe("Bad date");
  });

  test("a 5xx with an HTML body is a readable error, not the HTML", async () => {
    reply(502, "<html>bad gateway</html>", "text/html");
    const r = await api("/api/board");
    expect(r.error?.status).toBe(502);
    expect(r.error?.message).not.toContain("<html>");
    expect(r.error?.message).toContain("502");
  });

  test("a 200 that is not JSON (the page served for a missing route) is a bad_response error", async () => {
    reply(200, "<!doctype html><html></html>", "text/html");
    const r = await api("/api/nothing");
    expect(r.error?.code).toBe("bad_response");
  });

  test("network failure never throws: {error} with status 0", async () => {
    globalThis.fetch = (async () => {
      throw new TypeError("Failed to fetch");
    }) as unknown as typeof fetch;
    const r = await api("/api/board");
    expect(r.error).toMatchObject({ status: 0, code: "network" });
    expect(r.data).toBeUndefined();
  });

  test("an aborted request is {error} code aborted", async () => {
    globalThis.fetch = (async () => {
      throw new DOMException("aborted", "AbortError");
    }) as unknown as typeof fetch;
    expect((await api("/x")).error?.code).toBe("aborted");
  });
});

describe("session state", () => {
  const realFetch = globalThis.fetch;
  afterEach(() => {
    globalThis.fetch = realFetch;
    resetSession();
  });
  const reply = (status: number, body: string) => {
    globalThis.fetch = (async () => new Response(body, { status, headers: { "content-type": "application/json" } })) as unknown as typeof fetch;
  };

  test("starts idle; /api/auth/me with a person makes it ready", async () => {
    expect(getSessionState()).toEqual({ status: "idle" });
    reply(200, '{"login":"anna","role":"admin","id":3}');
    await loadSession();
    expect(getSessionState()).toEqual({ status: "ready", person: { login: "anna", role: "admin", id: 3 } });
  });

  test("it asks GET /api/auth/me", async () => {
    const seen: string[] = [];
    globalThis.fetch = (async (url: string) => {
      seen.push(String(url));
      return new Response("{}", { status: 401 });
    }) as unknown as typeof fetch;
    await loadSession();
    expect(seen).toEqual(["/api/auth/me"]);
  });

  test("401 and 403 mean nobody: anonymous", async () => {
    reply(401, '{"error":"no session"}');
    await loadSession();
    expect(getSessionState()).toEqual({ status: "anonymous" });
    reply(403, "{}");
    await loadSession();
    expect(getSessionState()).toEqual({ status: "anonymous" });
  });

  test("a server failure or an unreadable answer is an error, not 'logged out'", async () => {
    reply(500, "oops");
    await loadSession();
    expect(getSessionState().status).toBe("error");
    reply(200, '{"login":"anna","role":"root"}');
    await loadSession();
    expect(getSessionState().status).toBe("error");
    globalThis.fetch = (async () => {
      throw new TypeError("offline");
    }) as unknown as typeof fetch;
    await loadSession();
    expect(getSessionState()).toMatchObject({ status: "error", error: { code: "network" } });
  });

  test("a failed logout keeps the session; a done one ends it", async () => {
    reply(200, '{"login":"sam","role":"staff"}');
    await loadSession();
    reply(500, "oops");
    expect(await signOut()).not.toBeNull();
    expect(getSessionState().status).toBe("ready");
    reply(204, "");
    expect(await signOut()).toBeNull();
    expect(getSessionState()).toEqual({ status: "anonymous" });
  });

  test("a 401 from any other route ends a ready session", async () => {
    reply(200, '{"login":"sam","role":"staff"}');
    await loadSession();
    expect(getSessionState().status).toBe("ready");
    reply(401, "{}");
    await api("/api/admin/staff");
    expect(getSessionState()).toEqual({ status: "anonymous" });
  });

  test("a 401 from /api/auth/ (a wrong password) does not touch the session", async () => {
    reply(200, '{"login":"sam","role":"staff"}');
    await loadSession();
    reply(401, '{"error":"Wrong login or password"}');
    await api("/api/auth/login", { method: "POST", body: {} });
    expect(getSessionState().status).toBe("ready");
  });
});
