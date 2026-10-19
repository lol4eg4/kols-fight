import type { ReactNode } from "react";

export interface EmptyStateProps {
  /** One line: what happened and what to do. */
  message: string;
  /** Optional button or link for the next step. */
  action?: ReactNode;
}

/** Nothing to show. One line, no picture. */
export function EmptyState({ message, action }: EmptyStateProps) {
  return (
    <div className="ds-empty" role="status">
      <p className="ds-empty__message">{message}</p>
      {action && <div className="ds-empty__action">{action}</div>}
    </div>
  );
}
