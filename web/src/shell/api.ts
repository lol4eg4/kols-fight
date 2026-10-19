import { useCallback, useEffect, useState } from "react";
import { t } from "./i18n";

export interface ApiError {
  /** HTTP status, or 0 when no answer came (network down, aborted). */
  status: number;
  /** Text for the reader. Never a raw stack or HTML. */
  message: string;
  /** Machine code when the server sent one, or "network", "aborted", "bad_response". */
  code?: string;
}

/** Exactly one of data and error is set. Screens branch on `error`; api() never throws. */
export type ApiResult<T> = { data: T; error?: undefined } | { data?: undefined; error: ApiError };

export interface ApiInit {
  method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  /** Sent as JSON. */
  body?: unknown;
  signal?: AbortSignal;
}

type UnauthorizedListener = () => void;
const unauthorized = new Set<UnauthorizedListener>();

/** The session module listens here: a 401 outside /api/auth/ means the session ended. */
export function onUnauthorized(fn: UnauthorizedListener): () => void {
  unauthorized.add(fn);
  return () => unauthorized.delete(fn);
}

function errorFromBody(status: number, text: string): ApiError {
  let message: string | undefined;
  let code: string | undefined;
  try {
    const j = JSON.parse(text) as unknown;
    if (j && typeof j === "object") {
      const e = (j as Record<string, unknown>).error ?? j;
      if (typeof e === "string") message = e;
      else if (e && typeof e === "object") {
        const o = e as Record<string, unknown>;
        if (typeof o.message === "string") message = o.message;
        if (typeof o.code === "string") code = o.code;
      }
      if (!message && typeof (j as Record<string, unknown>).message === "string") message = (j as Record<string, string>).message;
    }
  } catch {
    // not JSON: use the status text below
  }
  if (message === undefined) message = status >= 500 ? t("shell.error.server", { status }) : t("shell.error.request", { status });
  return { status, message, code };
}

/**
 * Calls the server. Returns {data} or {error}; it never throws, so a screen has one path for every failure:
 * network down, non-2xx, a body that is not JSON.
 */
export async function api<T = unknown>(path: string, init: ApiInit = {}): Promise<ApiResult<T>> {
  try {
    const res = await fetch(path, {
      method: init.method ?? (init.body === undefined ? "GET" : "POST"),
      headers: { Accept: "application/json", ...(init.body === undefined ? {} : { "Content-Type": "application/json" }) },
      body: init.body === undefined ? undefined : JSON.stringify(init.body),
      credentials: "same-origin",
      signal: init.signal,
    });
    const text = await res.text();
    if (!res.ok) {
      if (res.status === 401 && !path.startsWith("/api/auth/")) unauthorized.forEach((fn) => fn());
      return { error: errorFromBody(res.status, text) };
    }
    if (text === "") return { data: undefined as T };
    try {
      return { data: JSON.parse(text) as T };
    } catch {
      return { error: { status: res.status, code: "bad_response", message: t("shell.error.bad_response") } };
    }
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") {
      return { error: { status: 0, code: "aborted", message: t("shell.error.aborted") } };
    }
    return { error: { status: 0, code: "network", message: t("shell.error.network") } };
  }
}

export type ApiState<T> = { status: "loading" } | { status: "ready"; data: T } | { status: "error"; error: ApiError };

/**
 * Loads `path` when the component mounts and whenever `path` changes. A null path loads nothing.
 * Returns the state and `reload` for a "Try again" button. An old answer never overwrites a newer request.
 */
export function useApi<T>(path: string | null): ApiState<T> & { reload: () => void } {
  const [state, setState] = useState<ApiState<T>>({ status: "loading" });
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (path === null) return;
    const ctl = new AbortController();
    setState({ status: "loading" });
    api<T>(path, { signal: ctl.signal }).then((r) => {
      if (ctl.signal.aborted) return;
      setState(r.error ? { status: "error", error: r.error } : { status: "ready", data: r.data as T });
    });
    return () => ctl.abort();
  }, [path, tick]);

  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { ...state, reload };
}
