import { lazy, Suspense, useEffect, useMemo, useRef } from "react";
import type { ComponentType, LazyExoticComponent, ReactNode } from "react";
import { EmptyState, ErrorLine, Skeleton, buttonClass } from "@arena/design";
import { ErrorBoundary } from "./slots";
import { Frame, TeamLayout } from "./frame";
import { t } from "./i18n";
import { Link, ParamsContext, navigate, useLocation } from "./nav";
import type { Location } from "./nav";
import { getRegistry } from "./plugin";
import type { ScreenEntry, ScreenLoader } from "./plugin";
import { HOME_PATH, isTeamPath, loginUrl, needsSession, resolveRoute, tagAllows } from "./route";
import type { Params, Resolved } from "./route";
import { useSession } from "./session";

const lazyCache = new Map<ScreenLoader, LazyExoticComponent<ComponentType>>();

function lazyScreen(load: ScreenLoader): LazyExoticComponent<ComponentType> {
  let c = lazyCache.get(load);
  if (!c) lazyCache.set(load, (c = lazy(load)));
  return c;
}

/** One screen: lazy-loaded (its own chunk), inside an error boundary and a Suspense with a Skeleton. */
function ScreenHost({ screen, params }: { screen: ScreenEntry; params: Params }) {
  const Comp = screen.load ? lazyScreen(screen.load) : null;
  return (
    <ParamsContext.Provider value={params}>
      <ErrorBoundary
        fallback={(retry) => (
          <ErrorLine
            message={t("shell.error.screen")}
            onRetry={() => {
              if (screen.load) lazyCache.delete(screen.load); // a failed chunk load must be tried again, not replayed
              retry();
            }}
            retryLabel={t("shell.retry")}
          />
        )}
      >
        <Suspense fallback={<Skeleton label={t("shell.loading")} variant="block" />}>
          {Comp ? <Comp /> : <EmptyState message={t("shell.not_built")} />}
        </Suspense>
      </ErrorBoundary>
    </ParamsContext.Provider>
  );
}

function NotFound() {
  return (
    <EmptyState
      message={t("shell.not_found")}
      action={
        <Link to="/" className={buttonClass("secondary")}>
          {t("shell.go_home")}
        </Link>
      }
    />
  );
}

function Redirect({ to }: { to: string }) {
  useEffect(() => navigate(to, { replace: true }), [to]);
  return null;
}

/** The /team rules: no session goes to the login with ?next=, a wrong role gets a "not allowed" line. */
function TeamGate({ screen, params }: { screen: ScreenEntry | null; params: Params }) {
  const { state, reload } = useSession();
  const loc = useLocation();
  switch (state.status) {
    case "idle":
    case "loading":
      return <Skeleton label={t("shell.loading")} variant="block" />;
    case "anonymous":
      return <Redirect to={loginUrl(loc.pathname + loc.search)} />;
    case "error":
      return <ErrorLine message={state.error.message} onRetry={reload} retryLabel={t("shell.retry")} />;
    case "ready": {
      if (screen && !tagAllows(screen.tag, state.person.role)) return <EmptyState message={t("shell.forbidden")} />;
      return <TeamLayout person={state.person}>{screen ? <ScreenHost screen={screen} params={params} /> : <NotFound />}</TeamLayout>;
    }
  }
}

/** Build-time switch: VITE_DESIGN_GALLERY_OPEN=1 lets /team/design render without a session. Off by default. */
function galleryOpen(): boolean {
  return (import.meta as ImportMeta & { env?: Record<string, string | undefined> }).env?.VITE_DESIGN_GALLERY_OPEN === "1";
}

function isGated(resolved: Resolved): boolean {
  if (resolved.kind === "notfound") return resolved.team;
  if (resolved.kind !== "screen") return false;
  return needsSession(resolved.screen) && !(galleryOpen() && resolved.screen.key === "shell/design");
}

function scrollToHash(hash: string): () => void {
  let id = hash.slice(1);
  try {
    id = decodeURIComponent(id);
  } catch {
    // keep the raw text
  }
  let tries = 0;
  let timer: number | undefined;
  const go = () => {
    const el = document.getElementById(id);
    if (el) el.scrollIntoView();
    else if (++tries < 12) timer = window.setTimeout(go, 150); // the section may still be loading
  };
  go();
  return () => window.clearTimeout(timer);
}

/** After a move to another page: back to the top and focus on the content. A hash scrolls to its section. */
function useRouteEffects(loc: Location, resolved: Resolved) {
  const prevBase = useRef<string | null>(null);
  const base = resolved.kind === "overlay" ? HOME_PATH : loc.pathname; // an overlay shares the home page under it
  useEffect(() => {
    if (loc.hash) return scrollToHash(loc.hash);
    if (prevBase.current !== null && prevBase.current !== base) {
      window.scrollTo(0, 0);
      document.getElementById("main")?.focus({ preventScroll: true });
    }
  }, [base, loc.hash]);
  useEffect(() => {
    prevBase.current = base;
  }, [base]);
}

function Content({ resolved }: { resolved: Resolved }): ReactNode {
  // Keys keep the home page mounted when an overlay opens over it, so it does not reload under the card.
  switch (resolved.kind) {
    case "redirect":
      return <Redirect to={resolved.to} />;
    case "notfound":
      return resolved.team ? <TeamGate screen={null} params={{}} /> : <NotFound />;
    case "overlay":
      return (
        <>
          {resolved.base ? (
            <ScreenHost key={resolved.base.key} screen={resolved.base} params={{}} />
          ) : (
            <EmptyState key="no-home" message={t("shell.not_built")} />
          )}
          <ScreenHost key={resolved.overlay.key} screen={resolved.overlay} params={resolved.params} />
        </>
      );
    case "screen":
      return isGated(resolved) ? (
        <TeamGate screen={resolved.screen} params={resolved.params} />
      ) : (
        <>
          <ScreenHost key={resolved.screen.key} screen={resolved.screen} params={resolved.params} />
        </>
      );
  }
}

/** Picks the screen for the address and puts it in the frame. */
export function Router() {
  const loc = useLocation();
  const registry = getRegistry();
  const resolved = useMemo(() => resolveRoute(registry.screens, loc.pathname), [registry, loc.pathname]);
  useRouteEffects(loc, resolved);
  return (
    <Frame variant={isTeamPath(loc.pathname) && isGated(resolved) ? "team" : "public"}>
      <Content resolved={resolved} />
    </Frame>
  );
}
