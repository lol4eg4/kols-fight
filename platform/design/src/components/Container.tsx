import type { ElementType, ReactNode } from "react";

export interface ContainerProps {
  /** Element to render. Default div. */
  as?: ElementType;
  children: ReactNode;
}

/** Content column: up to 1148px, centred, side margin 16px on a phone, 24px on a tablet, 32px wider. */
export function Container({ as: As = "div", children }: ContainerProps) {
  return <As className="ds-container">{children}</As>;
}
