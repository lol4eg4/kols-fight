import { createContext, useContext, useMemo, useSyncExternalStore } from "react";
import type { AnchorHTMLAttributes, MouseEvent } from "react";
import { HOME_PATH, OVERLAY_PATHS, matchPath } from "./route";
import type { Params } from "./route";

// History API navigation: navigate(), <Link>, useLocation() and the overlay close.

export interface Location {
  pathname: string;
  search: string;
  hash: string;
}

interface HistoryState {
  /** Steps taken inside the app since the page loaded. 0 means this page was opened directly. */
  idx: number;
  /** Path we came from inside the app. */
  prev?: string;
}

const NAV_EVENT = "arena:navigate";

function readState(): HistoryState {
  const s = window.history.state as Partial<HistoryState> | null;
  return { idx: typeof s?.idx === "number" ? s.idx : 0, prev: s?.prev };
}

/** Goes to a path inside the app without a page load. A link to another site is opened the normal way. */
export function navigate(to: string, opts: { replace?: boolean } = {}): void {
  const url = new URL(to, window.location.href);
  if (url.origin !== window.location.origin) {
    window.location.assign(to);
    return;
  }
  const next = url.pathname + url.search + url.hash;
  const here = window.location.pathname + window.location.search + window.location.hash;
  if (next === here) return;
  const cur = readState();
  if (opts.replace) window.history.replaceState({ ...cur }, "", next);
  else window.history.pushState({ idx: cur.idx + 1, prev: window.location.pathname } satisfies HistoryState, "", next);
  window.dispatchEvent(new Event(NAV_EVENT));
}

function subscribe(fn: () => void) {
  window.addEventListener("popstate", fn);
  window.addEventListener(NAV_EVENT, fn);
  return () => {
    window.removeEventListener("popstate", fn);
    window.removeEventListener(NAV_EVENT, fn);
  };
}

const snapshot = () => window.location.pathname + window.location.search + window.location.hash;

export function parseLocation(href: string): Location {
  const u = new URL(href, "http://x");
  return { pathname: u.pathname, search: u.search, hash: u.hash };
}

export function useLocation(): Location {
  const href = useSyncExternalStore(subscribe, snapshot, () => "/");
  return useMemo(() => parseLocation(href), [href]);
}

/** Query string of the current page. */
export function useSearchParams(): URLSearchParams {
  const { search } = useLocation();
  return useMemo(() => new URLSearchParams(search), [search]);
}

export const ParamsContext = createContext<Params>({});

/** Path parameters of the screen, for example { id } on /kol/:id. */
export function useParams<T extends Params = Params>(): T {
  return useContext(ParamsContext) as T;
}

/**
 * Closes an overlay (the KOL card, the fight window). Back to the home page it opened over; when the overlay
 * was opened by a direct link there is nothing to go back to, so it goes to the home page and replaces the entry.
 */
export function closeOverlay(): void {
  const { idx, prev } = readState();
  const cameFromHome = prev !== undefined && (prev === HOME_PATH || OVERLAY_PATHS.some((p) => matchPath(p, prev) !== null));
  if (idx > 0 && cameFromHome) window.history.back();
  else navigate(HOME_PATH, { replace: true });
}

/** For an overlay screen: the function for its Dialog's onClose (the × button, Esc, the dim background). */
export function useOverlayClose(): () => void {
  return closeOverlay;
}

export type LinkProps = Omit<AnchorHTMLAttributes<HTMLAnchorElement>, "href"> & { to: string };

/** A link inside the app: no page load, but still a real <a> (middle click, copy link, open in a new tab work). */
export function Link({ to, onClick, target, ...rest }: LinkProps) {
  const handle = (e: MouseEvent<HTMLAnchorElement>) => {
    onClick?.(e);
    if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    if ((target && target !== "_self") || rest.download !== undefined) return;
    if (new URL(to, window.location.href).origin !== window.location.origin) return;
    e.preventDefault();
    navigate(to);
  };
  return <a {...rest} href={to} target={target} onClick={handle} />;
}
