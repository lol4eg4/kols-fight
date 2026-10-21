import { Button } from "./Button";

export type ErrorLineProps = { message: string } & (
  | { onRetry?: undefined; retryLabel?: undefined }
  | { onRetry: () => void; retryLabel: string }
);

/** Something failed. One red line, and a retry button when retrying can help. Never shows old figures. */
export function ErrorLine({ message, onRetry, retryLabel }: ErrorLineProps) {
  return (
    <div className="ds-error" role="alert">
      <p className="ds-error__message">{message}</p>
      {onRetry && (
        <Button variant="secondary" onClick={onRetry}>
          {retryLabel}
        </Button>
      )}
    </div>
  );
}
