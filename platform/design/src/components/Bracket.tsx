import { useId } from "react";
import type { CSSProperties, ReactNode } from "react";
import { cx } from "../cx";
import { PixelAvatar } from "./PixelAvatar";

export interface BracketEntry {
  /** KOL id, for the avatar. */
  id: string;
  name: string;
  /** Score or any small figure shown at the right. */
  score?: ReactNode;
  /** Won this match: yellow frame and a star. */
  winner?: boolean;
}

export interface BracketMatch {
  id: string;
  /** null while the slot waits for an earlier match. */
  a: BracketEntry | null;
  b: BracketEntry | null;
}

export interface BracketRound {
  id: string;
  /** For example "Quarterfinals". */
  title: string;
  matches: BracketMatch[];
}

export interface BracketProps {
  /** Rounds in play order; each round has half the matches of the one before. */
  rounds: BracketRound[];
  /** Names the bracket for screen readers. */
  label: string;
  /** Text for a slot that is not decided yet. */
  pendingLabel: string;
  /** Read after the star: "winner". */
  winnerLabel: string;
}

function Entry({ entry, pendingLabel, winnerLabel }: { entry: BracketEntry | null; pendingLabel: string; winnerLabel: string }) {
  if (!entry) {
    return <li className="ds-bracket__entry ds-bracket__entry--pending">{pendingLabel}</li>;
  }
  return (
    <li className={cx("ds-bracket__entry", entry.winner && "ds-bracket__entry--winner")}>
      <PixelAvatar id={entry.id} size={24} />
      <span className="ds-bracket__name">{entry.name}</span>
      {entry.score !== undefined && <span className="ds-bracket__score">{entry.score}</span>}
      {entry.winner && (
        <span className="ds-bracket__star">
          <span aria-hidden="true">★</span>
          <span className="ds-sr-only">{winnerLabel}</span>
        </span>
      )}
    </li>
  );
}

/** Tournament bracket: a tree on desktop (rounds in columns), rounds stacked on a phone. */
export function Bracket({ rounds, label, pendingLabel, winnerLabel }: BracketProps) {
  const base = useId();
  return (
    <div className="ds-bracket" role="group" aria-label={label} style={{ "--rounds": rounds.length } as CSSProperties}>
      {rounds.map((round) => (
        <section key={round.id} className="ds-bracket__round" aria-labelledby={`${base}-${round.id}`}>
          <h3 id={`${base}-${round.id}`} className="ds-bracket__title">
            {round.title}
          </h3>
          <div className="ds-bracket__matches">
            {round.matches.map((m) => (
              <div key={m.id} className="ds-bracket__slot">
                <ul className="ds-bracket__match">
                  <Entry entry={m.a} pendingLabel={pendingLabel} winnerLabel={winnerLabel} />
                  <Entry entry={m.b} pendingLabel={pendingLabel} winnerLabel={winnerLabel} />
                </ul>
              </div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
