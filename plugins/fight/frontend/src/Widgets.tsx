import { useEffect, useRef, useState } from "react";
import { Button } from "@arena/design";
import { t, useSearchParams } from "@arena/shell";
import type { WidgetProps } from "@arena/shell";
import { byKol, useBoard } from "./board";
import { boardDate, isFull } from "./logic";
import { Picker } from "./Picker";
import { addPending, removePending, useFightStore } from "./store";

/** "⚔️ Fight" on a board row or a KOL card: puts the KOL in the pending fight, or takes them out again. */
export function AddToFight({ kol }: WidgetProps<"fight/add_to_fight">) {
  const { pending } = useFightStore();
  const board = useBoard(boardDate(useSearchParams().get("date")));
  const name = board.status === "ready" ? (byKol(board.data.rows).get(kol)?.name ?? kol) : kol;
  const inFight = pending.includes(kol);
  const label = (
    <>
      <span aria-hidden="true">{inFight ? "✓ " : "⚔️ "}</span>
      <span className="fight-add__word">{t("fight.add.button")}</span>
      <span className="ds-sr-only"> {t("fight.add.who", { name })}</span>
    </>
  );
  if (!inFight && isFull(pending)) {
    return (
      <span className="fight-add">
        <Button variant="secondary" disabled reason={t("fight.add.full")}>
          {label}
        </Button>
      </span>
    );
  }
  return (
    <span className={inFight ? "fight-add fight-add--on" : "fight-add"}>
      <Button variant="secondary" aria-pressed={inFight} onClick={() => (inFight ? removePending(kol) : addPending(kol))}>
        {label}
      </Button>
    </span>
  );
}

/** The compact picker on the home page. FIGHT opens the fight window. */
export function FighterPicker(_: WidgetProps<"fight/fighter_picker">) {
  const { pending } = useFightStore();
  const ref = useRef<HTMLDivElement>(null);
  const [inView, setInView] = useState(true);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const io = new IntersectionObserver(([e]) => setInView(!!e?.isIntersecting));
    io.observe(el);
    return () => io.disconnect();
  }, []);
  // Fighters picked on the board, far from the picker: a bar at the bottom leads back to it.
  const toPicker = () => {
    const el = ref.current;
    el?.scrollIntoView({ block: "start" });
    // Two or more picked: focus FIGHT. Otherwise the search, to pick the next one.
    (el?.querySelector<HTMLElement>(".fight-picker__go .ds-btn--fight:not(:disabled):not([aria-disabled='true'])") ?? el?.querySelector<HTMLElement>("input"))?.focus();
  };
  return (
    <>
      <div className="fight-home" ref={ref}>
        <Picker headingLevel={3} />
      </div>
      {pending.length > 0 && !inView && (
        <div className="fight-dock" role="region" aria-label={t("fight.dock.label")}>
          <span className="fight-dock__count">{t("fight.picker.count", { n: pending.length })}</span>
          <Button variant="secondary" onClick={toPicker}>
            {t("fight.dock.go")}
          </Button>
        </div>
      )}
    </>
  );
}
