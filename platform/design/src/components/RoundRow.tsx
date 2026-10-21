import type { ReactNode } from "react";
import { cx } from "../cx";

export interface RoundFighter {
  id: string;
  name: string;
  /** The value as the reader should see it. null means this fighter has no data. */
  value: ReactNode | null;
  /** Bar length against the best value, 0 to 1. null for no data. */
  share: number | null;
  /** Took the round (or a tied share of it): yellow bar and a star. */
  winner?: boolean;
}

export interface RoundRowProps {
  number: number;
  /** For example "Round 2". */
  numberLabel: string;
  /** What is compared, for example "PnL 30 days". */
  title: string;
  /** What we measure and why. */
  measures?: string;
  /**
   * normal: has a winner. void: not counted, the row is grey (50%) and says why.
   * tie: the best value is shared, the point is split.
   */
  status?: "normal" | "void" | "tie";
  /** Text for a void or tie round, for example "not counted: not everyone has data". */
  statusLabel?: string;
  /** Value text for a fighter with no data: "no data". */
  noDataLabel: string;
  /** Read after the star: "winner". */
  winnerLabel: string;
  /** One line on why the winner won, for example "Decu +$565K vs Tekkerrss +$207K: 2.7× more". */
  why?: string;
  /** If set, announced once to screen readers when the row appears: "Round 2, PnL 30 days: Decu wins ...". */
  announce?: string;
  fighters: RoundFighter[];
}

/** One round of a fight: title, what is measured, a bar per fighter, the winner marked with a star. */
export function RoundRow({ number, numberLabel, title, measures, status = "normal", statusLabel, noDataLabel, winnerLabel, why, announce, fighters }: RoundRowProps) {
  return (
    <section className={cx("ds-round", `ds-round--${status}`)} data-round={number}>
      <div className="ds-round__head">
        <p className="ds-round__num">{numberLabel}</p>
        <h3 className="ds-round__title">{title}</h3>
        {measures && <p className="ds-round__measures">{measures}</p>}
        {status !== "normal" && statusLabel && <p className="ds-round__status">{statusLabel}</p>}
      </div>
      <ul className="ds-round__fighters">
        {fighters.map((f) => {
          const empty = f.value === null;
          return (
            <li key={f.id} className={cx("ds-round__fighter", f.winner && "ds-round__fighter--winner", empty && "ds-round__fighter--empty")}>
              <span className="ds-round__name">{f.name}</span>
              <span className="ds-round__bar" aria-hidden="true">
                <span className="ds-round__fill" style={{ width: `${Math.max(empty ? 0 : (f.share ?? 0) * 100, 2)}%` }} />
              </span>
              <span className="ds-round__value">{empty ? noDataLabel : f.value}</span>
              <span className="ds-round__star">
                {f.winner && (
                  <>
                    <span aria-hidden="true">★</span>
                    <span className="ds-sr-only">{winnerLabel}</span>
                  </>
                )}
              </span>
            </li>
          );
        })}
      </ul>
      {why && <p className="ds-round__why">{why}</p>}
      {announce && (
        <p className="ds-sr-only" role="status">
          {announce}
        </p>
      )}
    </section>
  );
}
