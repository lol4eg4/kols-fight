import { createContext, useCallback, useContext, useEffect, useMemo, useState } from "react";
import type { ReactNode } from "react";
import { Button, Container, ErrorLine, Logo, Select, Ticker } from "@arena/design";
import type { TickerItem } from "@arena/design";
import { t } from "./i18n";
import { Link, navigate, useLocation } from "./nav";
import { getRegistry } from "./plugin";
import { ANCHORS } from "./route";
import { signOut } from "./session";
import type { Person } from "./session";
import { Slot } from "./slots";

// ---------- ticker ----------

type SetTickerItems = (owner: string, items: TickerItem[]) => void;
const TickerContext = createContext<SetTickerItems>(() => {});

/**
 * Adds items to the ticker strip at the top of the frame. `owner` is the plugin name; call it again to replace
 * that plugin's items. Memoize `items` with useMemo: a new array every render restarts the update.
 */
export function useTickerItems(owner: string, items: TickerItem[]): void {
  const set = useContext(TickerContext);
  useEffect(() => {
    set(owner, items);
    return () => set(owner, []);
  }, [set, owner, items]);
}

function sameItems(a: TickerItem[] | undefined, b: TickerItem[]): boolean {
  return !!a && a.length === b.length && a.every((x, i) => x.id === b[i]!.id && x.content === b[i]!.content);
}

function TickerBand({ byOwner }: { byOwner: Record<string, TickerItem[]> }) {
  // The board's own items first, then the fillers' (moves, ...) in name order.
  const items = useMemo(
    () =>
      Object.keys(byOwner)
        .sort((a, b) => (a === "board" ? -1 : b === "board" ? 1 : a.localeCompare(b)))
        .flatMap((owner) => byOwner[owner]!.map((i) => ({ ...i, id: `${owner}:${i.id}` }))),
    [byOwner],
  );
  return (
    <div className="frame__ticker">
      {/* Fillers render nothing themselves; they add items through useTickerItems. */}
      <Slot name="board/ticker" props={{}} loading={null} />
      <Ticker items={items} label={t("shell.ticker.label")} pauseLabel={t("shell.ticker.pause")} resumeLabel={t("shell.ticker.resume")} />
    </div>
  );
}

// ---------- header ----------

const NAV: ReadonlyArray<{ anchor: string; long: () => string; short: () => string }> = [
  { anchor: ANCHORS.leaderboard, long: () => t("shell.nav.leaderboard"), short: () => t("shell.nav.leaderboard_short") },
  { anchor: ANCHORS.network, long: () => t("shell.nav.network"), short: () => t("shell.nav.network_short") },
  { anchor: ANCHORS.fight, long: () => t("shell.nav.fight"), short: () => t("shell.nav.fight_short") },
  { anchor: ANCHORS.score, long: () => t("shell.nav.score"), short: () => t("shell.nav.score_short") },
];

function PublicHeader() {
  return (
    <header className="frame__header">
      <Container>
        <div className="frame__bar">
          <Link to="/" className="frame__logo">
            <span className="frame__logo-full">
              <Logo label={t("shell.logo")} />
            </span>
            <span className="frame__logo-icon">
              <Logo label={t("shell.logo")} wordmark={false} size={32} />
            </span>
          </Link>
          <nav className="frame__nav" aria-label={t("shell.nav.label")}>
            {NAV.map((n) => (
              <Link key={n.anchor} to={`/#${n.anchor}`} className="frame__link">
                <span className="frame__long">{n.long()}</span>
                <span className="frame__short">{n.short()}</span>
              </Link>
            ))}
          </nav>
        </div>
      </Container>
    </header>
  );
}

function TeamHeader() {
  return (
    <header className="frame__header">
      <Container>
        <div className="frame__bar">
          <Link to="/team" className="frame__logo">
            <span className="frame__logo-full">
              <Logo label={t("shell.logo")} />
            </span>
            <span className="frame__logo-icon">
              <Logo label={t("shell.logo")} wordmark={false} size={32} />
            </span>
          </Link>
          <span className="frame__section">{t("shell.team.title")}</span>
          <Link to="/" className="frame__link frame__client">
            {t("shell.team.client_page")}
          </Link>
        </div>
      </Container>
    </header>
  );
}

// ---------- frame ----------

export interface FrameProps {
  /** public: ticker, header with the four anchors, "Skip to leaderboard". team: closed section header, "Skip to content". */
  variant: "public" | "team";
  children: ReactNode;
}

/** The page frame: skip link, ticker, sticky header, main. */
export function Frame({ variant, children }: FrameProps) {
  const [byOwner, setByOwner] = useState<Record<string, TickerItem[]>>({});
  const set = useCallback<SetTickerItems>((owner, items) => {
    setByOwner((prev) => {
      if (sameItems(prev[owner], items) || (items.length === 0 && !(owner in prev))) return prev;
      const next = { ...prev };
      if (items.length === 0) delete next[owner];
      else next[owner] = items;
      return next;
    });
  }, []);

  const team = variant === "team";
  return (
    <TickerContext.Provider value={set}>
      <a className="skip-link" href={team ? "#content" : `#${ANCHORS.leaderboard}`}>
        {team ? t("shell.skip.content") : t("shell.skip.leaderboard")}
      </a>
      <TickerBand byOwner={byOwner} />
      {team ? <TeamHeader /> : <PublicHeader />}
      <main id="main" tabIndex={-1} className="frame__main">
        {children}
      </main>
    </TickerContext.Provider>
  );
}

// ---------- team layout ----------

const TEAM_ORDER = ["wallets", "network", "fomo", "picker", "collection", "staff"];

const TEAM_LABELS: Record<string, () => string> = {
  wallets: () => t("shell.team.nav.wallets"),
  network: () => t("shell.team.nav.network"),
  fomo: () => t("shell.team.nav.fomo"),
  picker: () => t("shell.team.nav.picker"),
  collection: () => t("shell.team.nav.collection"),
  staff: () => t("shell.team.nav.staff"),
};

interface NavItem {
  name: string;
  path: string;
  label: string;
}

/** Menu entries from the screens plugins declare under /team: Wallets, Network, FOMO match, Picker, Collection, Staff. */
function teamNav(person: Person): NavItem[] {
  const items = getRegistry()
    .screens.filter((s) => s.path.startsWith("/team/") && s.tag !== "public" && s.plugin !== "shell")
    .filter((s) => s.tag !== "admin" || person.role === "admin")
    .map((s) => ({ name: s.name, path: s.path, label: TEAM_LABELS[s.name]?.() ?? s.name }));
  const rank = (n: string) => (TEAM_ORDER.includes(n) ? TEAM_ORDER.indexOf(n) : TEAM_ORDER.length);
  return items.sort((a, b) => rank(a.name) - rank(b.name) || a.name.localeCompare(b.name));
}

/** Closed section: menu on the left (a drop-down on a phone), your login, sign out, and the screen. */
export function TeamLayout({ person, children }: { person: Person; children: ReactNode }) {
  const { pathname } = useLocation();
  const items = useMemo(() => teamNav(person), [person]);
  const current = items.find((i) => i.path === pathname);
  const [signOutFailed, setSignOutFailed] = useState(false);
  const leave = async () => setSignOutFailed((await signOut()) !== null);

  return (
    <Container>
      <div className="team">
        <aside className="team__side">
          <nav className="team__menu" aria-label={t("shell.team.menu")}>
            {items.map((i) => (
              <Link key={i.path} to={i.path} className="team__link" aria-current={i.path === pathname ? "page" : undefined}>
                {i.label}
              </Link>
            ))}
          </nav>
          <div className="team__phone-menu">
            <Select
              label={t("shell.team.section")}
              value={current?.path ?? ""}
              placeholder={current ? undefined : t("shell.team.section_pick")}
              options={items.map((i) => ({ value: i.path, label: i.label }))}
              onChange={(e) => navigate(e.target.value)}
            />
          </div>
          <div className="team__who">
            <span className="team__login">{t("shell.team.signed_in", { login: person.login })}</span>
            <Button variant="secondary" onClick={() => void leave()}>
              {t("shell.team.sign_out")}
            </Button>
            {signOutFailed && <ErrorLine message={t("shell.team.sign_out_failed")} />}
          </div>
        </aside>
        <div id="content" className="team__content">
          {children}
        </div>
      </div>
    </Container>
  );
}
