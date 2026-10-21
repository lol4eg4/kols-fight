import { useState } from "react";
import { Button, Card, Heading, PixelAvatar } from "@arena/design";
import { t, usePageEvent } from "@arena/shell";
import type { SlotProps } from "@arena/shell";
import { choose, fightKey, load, save, settle } from "./storage";
import type { Saved, Verdict } from "./storage";

/** Storage state: `saved` is what we know; `kept` is false when the browser would not keep it. */
interface Mem {
  saved: Saved;
  kept: boolean;
}

function start(): Mem {
  const saved = load();
  return saved ? { saved, kept: true } : { saved: { called: 0, played: 0, picks: {} }, kept: false };
}

function keep(next: Saved, prev: Mem): Mem {
  return { saved: next, kept: prev.kept && save(next) };
}

function verdictText(v: Verdict): string {
  switch (v) {
    case "called":
      return t("guess.called");
    case "missed":
      return t("guess.missed");
    case "draw":
      return t("guess.draw");
  }
}

/** The pick before the fight; after `fight_over`, whether the visitor was right and the running score. */
export function Guess({ ids, date }: SlotProps<"fight/before_fight">) {
  const [mem, setMem] = useState<Mem>(start);
  const key = fightKey(ids, date);

  usePageEvent("fight_over", (e) => {
    if (fightKey(e.ids, e.date) !== key) return;
    setMem((m) => keep(settle(m.saved, key, e.winner).saved, m));
  });

  if (ids.length < 2) return null;
  const pick = mem.saved.picks[key];
  const verdict = pick?.verdict ?? null;
  const { called, played } = mem.saved;

  return (
    <Card as="section" aria-label={t("guess.title")}>
      <Heading level={3} variant="title">
        {t("guess.title")}
      </Heading>
      {verdict === null ? (
        <>
          <p>{t("guess.hint")}</p>
          <div role="group" aria-label={t("guess.title")}>
            {ids.map((id) => (
              <Button
                key={id}
                variant={pick?.kol === id ? "primary" : "secondary"}
                aria-pressed={pick?.kol === id}
                aria-label={t("guess.pick", { name: id })}
                onClick={() => setMem((m) => keep(choose(m.saved, key, id), m))}
              >
                <PixelAvatar id={id} size={24} /> {id}
              </Button>
            ))}
          </div>
          {pick && <p role="status">{t("guess.picked", { name: pick.kol })}</p>}
        </>
      ) : (
        <p role="status">
          <strong>{verdictText(verdict)}</strong>
        </p>
      )}
      {mem.kept && played > 0 && <p>{t("guess.score", { called, played })}</p>}
    </Card>
  );
}

export default Guess;
