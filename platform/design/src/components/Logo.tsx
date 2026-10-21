import { cx } from "../cx";

// The coin with sunglasses from the prototype, 12x12 cells. Y coin, K lens, W glare, . empty.
const COIN = [
  "...YYYYYY...",
  ".YYYYYYYYYY.",
  ".YYYYYYYYYY.",
  "YYYYYYYYYYYY",
  "YKKKKKKKKKKY",
  "YKWKKYYKWKKY",
  "YYKKKYYKKKYY",
  "YYYYYYYYYYYY",
  "YYKYYYYYYKYY",
  ".YYKKKKKKYY.",
  ".YYYYYYYYYY.",
  "...YYYYYY...",
];

function path(ch: string): string {
  let d = "";
  COIN.forEach((row, r) => [...row].forEach((c, i) => c === ch && (d += `M${i} ${r}h1v1h-1z`)));
  return d;
}

export interface LogoProps {
  /** Draw the KOL ARENA wordmark next to the coin. The phone header hides it. */
  wordmark?: boolean;
  /** Accessible name of the whole logo, for example "KOL Arena". */
  label: string;
  size?: number;
}

/** The coin is the logo; the wordmark is pixel text. */
export function Logo({ wordmark = true, label, size = 28 }: LogoProps) {
  return (
    <span className={cx("ds-logo", wordmark && "ds-logo--wordmark")}>
      <svg className="ds-logo__coin" width={size} height={size} viewBox="0 0 12 12" shapeRendering="crispEdges" role="img" aria-label={label}>
        <path d={path("Y")} className="ds-logo__y" />
        <path d={path("K")} className="ds-logo__k" />
        <path d={path("W")} className="ds-logo__w" />
      </svg>
      {wordmark && (
        <span className="ds-logo__text" aria-hidden="true">
          KOL ARENA
        </span>
      )}
    </span>
  );
}
