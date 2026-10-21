import { avatarPath, avatarSpec } from "../avatar";

export interface PixelAvatarProps {
  /** The KOL id. The sprite and its colour come from this alone. */
  id: string;
  /** Side in pixels. Default 40. */
  size?: number;
}

/** Decorative sprite on a dark tile. The name next to it carries the meaning, so it is hidden from screen readers. */
export function PixelAvatar({ id, size = 40 }: PixelAvatarProps) {
  const { cells, color } = avatarSpec(id);
  return (
    <svg
      className="ds-avatar"
      width={size}
      height={size}
      viewBox="0 0 7 7"
      shapeRendering="crispEdges"
      aria-hidden="true"
      focusable="false"
      data-color={color}
    >
      <rect x="0.1" y="0.1" width="6.8" height="6.8" rx="0.6" className="ds-avatar__tile" />
      <path d={avatarPath(cells)} className={`ds-avatar__sprite ds-avatar__sprite--${color}`} />
    </svg>
  );
}
