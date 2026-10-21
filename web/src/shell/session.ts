import { useCallback, useEffect, useSyncExternalStore } from "react";
import { api, onUnauthorized } from "./api";
import type { ApiError } from "./api";
import { t } from "./i18n";
import type { Role } from "./route";

/** Who is signed in. `GET /api/auth/me` answers {login, role} for a person with a session, 401 for nobody. */
export interface Person {
  login: string;
  role: Role;
  id?: number;
}

export type SessionState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "anonymous" }
  | { status: "ready"; person: Person }
  | { status: "error"; error: ApiError };

let state: SessionState = { status: "idle" };
const listeners = new Set<() => void>();

function set(next: SessionState) {
  state = next;
  listeners.forEach((l) => l());
}

function parsePerson(data: unknown): Person | null {
  if (!data || typeof data !== "object") return null;
  const d = data as Record<string, unknown>;
  const who = (d.person && typeof d.person === "object" ? d.person : d) as Record<string, unknown>;
  const role = who.role;
  if (typeof who.login !== "string" || (role !== "staff" && role !== "admin")) return null;
  return { login: who.login, role, id: typeof who.id === "number" ? who.id : undefined };
}

/** Asks the server who is signed in. Called by the first /team screen that needs it, and by `reload`. */
export async function loadSession(): Promise<void> {
  if (state.status === "loading") return;
  set({ status: "loading" });
  const r = await api<unknown>("/api/auth/me");
  if (r.error) {
    set(r.error.status === 401 || r.error.status === 403 ? { status: "anonymous" } : { status: "error", error: r.error });
    return;
  }
  const person = parsePerson(r.data);
  set(person ? { status: "ready", person } : { status: "error", error: { status: 200, code: "bad_response", message: t("shell.error.bad_response") } });
}

/** Ends the session on the server and forgets it here. The next /team page goes to the login. */
export async function signOut(): Promise<void> {
  await api("/api/auth/logout", { method: "POST" });
  set({ status: "anonymous" });
}

// Any 401 from a route outside /api/auth/ means the session ended: the next render sends the visitor to the login.
onUnauthorized(() => {
  if (state.status === "ready") set({ status: "anonymous" });
});

function subscribe(fn: () => void) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Session state for /team screens. The first call starts the check. */
export function useSession(): { state: SessionState; reload: () => void; signOut: () => Promise<void> } {
  const s = useSyncExternalStore(subscribe, () => state, () => state);
  useEffect(() => {
    if (s.status === "idle") void loadSession();
  }, [s.status]);
  const reload = useCallback(() => void loadSession(), []);
  return { state: s, reload, signOut };
}

/** Current state, for code outside React and for tests. */
export function getSessionState(): SessionState {
  return state;
}

/** For tests: back to the start. */
export function resetSession(): void {
  set({ status: "idle" });
}
