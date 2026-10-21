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
  return (
    <div className="fight-home">
      <Picker headingLevel={3} />
    </div>
  );
}
