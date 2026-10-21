import { useId, useMemo, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
import { Button, Card, ErrorLine, Field, PixelAvatar, Skeleton, TierBadge } from "@arena/design";
import { navigate, t } from "@arena/shell";
import { byKol, useBoard } from "./board";
import { MAX_FIGHTERS, canFight, fightPath, isFull, networkKols, randomPick, searchKols, topTwo } from "./logic";
import { addPending, clearPending, removePending, useFightStore } from "./store";
import type { BoardKol } from "./types";

export interface PickerProps {
  /** Heading level of the picker title in the page outline. */
  headingLevel?: 2 | 3;
  /** Inside the fight window: the fight replaces the picker in history, so closing goes home, not back to the picker. */
  replace?: boolean;
}

/** Up to 4 fighter slots, a search over the board, quick fights and the FIGHT button. Leads to /fight?ids=. */
export function Picker({ headingLevel = 3, replace = false }: PickerProps) {
  const board = useBoard();
  const { pending, last } = useFightStore();
  const [query, setQuery] = useState("");
  const [said, setSaid] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const titleId = useId();
  const H = headingLevel === 2 ? "h2" : "h3";

  const rows = board.status === "ready" ? board.data.rows : [];
  const rowOf = useMemo(() => byKol(rows), [rows]);
  const results = useMemo(() => searchKols(rows, query), [rows, query]);
  const network = useMemo(() => networkKols(rows), [rows]);
  const full = isFull(pending);

  const nameOf = (kol: string) => rowOf.get(kol)?.name ?? kol;

  const add = (row: BoardKol) => {
    if (pending.includes(row.kol) || full) return;
    addPending(row.kol);
    setSaid(t("fight.picker.added", { name: row.name, n: pending.length + 1 }));
    setQuery("");
    searchRef.current?.focus();
  };

  const remove = (kol: string) => {
    removePending(kol);
    setSaid(t("fight.picker.removed", { name: nameOf(kol), n: pending.length - 1 }));
    searchRef.current?.focus();
  };

  const go = (ids: readonly string[], date?: string | null) => navigate(fightPath(ids, date), { replace });

  const onSearchKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      const first = results.find((r) => !pending.includes(r.kol));
      if (first && !full) add(first);
    } else if (e.key === "ArrowDown") {
      const first = listRef.current?.querySelector<HTMLButtonElement>("button");
      if (first) {
        e.preventDefault();
        first.focus();
      }
    }
  };

  const onListKey = (e: KeyboardEvent<HTMLUListElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    const buttons = [...(listRef.current?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (at < 0) return;
    e.preventDefault();
    if (e.key === "ArrowUp" && at === 0) searchRef.current?.focus();
    else buttons[Math.min(Math.max(at + (e.key === "ArrowDown" ? 1 : -1), 0), buttons.length - 1)]?.focus();
  };

  const slots = Array.from({ length: MAX_FIGHTERS }, (_, i) => pending[i] ?? null);

  return (
    <div className="fight-picker" aria-labelledby={titleId} role="group">
      <div className="fight-picker__head">
        <H id={titleId} className="fight-picker__title">
          {t("fight.picker.title")}
        </H>
        <span className="fight-muted">{t("fight.picker.count", { n: pending.length })}</span>
      </div>

      <ul className="fight-slots" aria-label={t("fight.picker.slots")}>
        {slots.map((kol, i) =>
          kol === null ? (
            <li key={`empty-${i}`} className="fight-slot fight-slot--empty">
              <button type="button" className="fight-slot__empty" onClick={() => searchRef.current?.focus()} aria-label={t("fight.picker.slot_empty_label", { n: i + 1 })}>
                {t("fight.picker.slot_empty")}
              </button>
            </li>
          ) : (
            <li key={kol} className="fight-slot">
              <Card tone="fighter">
                <div className="fight-slot__body">
                  <PixelAvatar id={kol} size={40} />
                  <div className="fight-slot__who">
                    <span className="fight-slot__name">{nameOf(kol)}</span>
                    <SlotFacts row={rowOf.get(kol)} ready={board.status === "ready"} />
                  </div>
                  <button type="button" className="fight-x" onClick={() => remove(kol)} aria-label={t("fight.picker.remove", { name: nameOf(kol) })}>
                    <span aria-hidden="true">×</span>
                  </button>
                </div>
              </Card>
            </li>
          ),
        )}
      </ul>

      {board.status === "loading" && <Skeleton label={t("fight.board.loading")} variant="lines" count={2} />}
      {board.status === "error" && (
        <ErrorLine message={t("fight.board.error", { message: board.error.message })} onRetry={board.reload} retryLabel={t("fight.retry")} />
      )}
      {board.status === "ready" && rows.length === 0 && <p className="fight-muted">{t("fight.picker.empty_board")}</p>}

      {board.status === "ready" && rows.length > 0 && (
        <>
          <div className="fight-search">
            <Field
              ref={searchRef}
              label={t("fight.picker.search")}
              hint={t("fight.picker.search_hint")}
              type="search"
              autoComplete="off"
              spellCheck={false}
              value={query}
              onChange={(e) => setQuery(e.currentTarget.value)}
              onKeyDown={onSearchKey}
            />
            {query.trim() !== "" &&
              (results.length === 0 ? (
                <p className="fight-muted">{t("fight.picker.no_match", { query: query.trim() })}</p>
              ) : (
                <ul ref={listRef} className="fight-results" aria-label={t("fight.picker.results")} onKeyDown={onListKey}>
                  {results.map((r) => {
                    const picked = pending.includes(r.kol);
                    const off = picked || full;
                    return (
                      <li key={r.kol}>
                        <button type="button" className="fight-result" aria-disabled={off || undefined} onClick={() => !off && add(r)}>
                          <PixelAvatar id={r.kol} size={32} />
                          <span className="fight-result__name">{r.name}</span>
                          <span className="fight-result__facts">
                            <TierBadge tier={r.tier} />
                            <span className="fight-mono">{t("fight.picker.rank", { rank: r.rank })}</span>
                          </span>
                          {off && <span className="fight-result__off">{picked ? t("fight.picker.in_fight") : t("fight.picker.full_short")}</span>}
                        </button>
                      </li>
                    );
                  })}
                </ul>
              ))}
          </div>

          <div className="fight-quick" role="group" aria-label={t("fight.picker.quick")}>
            <Button variant="secondary" onClick={() => go(randomPick(rows, 2))}>
              {t("fight.quick.random")}
            </Button>
            <Button variant="secondary" onClick={() => go(topTwo(rows))}>
              {t("fight.quick.top")}
            </Button>
            {network.length >= 2 ? (
              <Button variant="secondary" onClick={() => go(randomPick(network, 2))}>
                {t("fight.quick.network")}
              </Button>
            ) : (
              <Button variant="secondary" disabled reason={t("fight.quick.network_off")}>
                {t("fight.quick.network")}
              </Button>
            )}
            {last ? (
              <Button variant="secondary" onClick={() => go(last.ids, last.date)}>
                {t("fight.quick.rematch")}
              </Button>
            ) : (
              <Button variant="secondary" disabled reason={t("fight.quick.rematch_off")}>
                {t("fight.quick.rematch")}
              </Button>
            )}
          </div>
        </>
      )}

      <div className="fight-picker__go">
        {canFight(pending) ? (
          <Button variant="fight" onClick={() => go(pending)}>
            {t("fight.picker.go")}
          </Button>
        ) : (
          <Button variant="fight" disabled reason={t("fight.picker.go_off")}>
            {t("fight.picker.go")}
          </Button>
        )}
        {pending.length > 0 && (
          <Button
            variant="secondary"
            onClick={() => {
              clearPending();
              setSaid("");
              searchRef.current?.focus();
            }}
          >
            {t("fight.picker.clear")}
          </Button>
        )}
      </div>

      <p className="ds-sr-only" role="status">
        {said}
      </p>
    </div>
  );
}

function SlotFacts({ row, ready }: { row: BoardKol | undefined; ready: boolean }) {
  if (!row) return ready ? <span className="fight-slot__unknown">{t("fight.picker.unknown")}</span> : null;
  return (
    <span className="fight-slot__facts">
      <TierBadge tier={row.tier} />
      <span className="fight-mono">{t("fight.picker.score", { score: Math.round(row.score) })}</span>
    </span>
  );
}
