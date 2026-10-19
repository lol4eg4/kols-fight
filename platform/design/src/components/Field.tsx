import { useId } from "react";
import type { InputHTMLAttributes, Ref, TextareaHTMLAttributes } from "react";
import { cx } from "../cx";

interface FieldBase {
  label: string;
  /** Keeps the label for screen readers but does not draw it (a search box next to its own icon). */
  hideLabel?: boolean;
  hint?: string;
  /** Error text under the field. Sets the red border and aria-invalid. */
  error?: string;
}

export type FieldProps = FieldBase &
  (
    | ({ multiline?: false; ref?: Ref<HTMLInputElement> } & Omit<InputHTMLAttributes<HTMLInputElement>, "className">)
    | ({ multiline: true; ref?: Ref<HTMLTextAreaElement> } & Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className">)
  );

/** Labelled text input. Error and hint are tied to the control with aria-describedby. */
export function Field(props: FieldProps) {
  const { label, hideLabel, hint, error, multiline, id: idProp, ...rest } = props;
  const auto = useId();
  const id = idProp ?? auto;
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy = [error && errorId, hint && hintId].filter(Boolean).join(" ") || undefined;
  const common = {
    id,
    className: "ds-input",
    "aria-invalid": error ? true : undefined,
    "aria-describedby": describedBy,
  } as const;
  return (
    <div className={cx("ds-field", error && "ds-field--invalid")}>
      <label htmlFor={id} className={cx("ds-field__label", hideLabel && "ds-sr-only")}>
        {label}
      </label>
      {multiline ? (
        <textarea {...(rest as TextareaHTMLAttributes<HTMLTextAreaElement>)} {...common} rows={(rest as TextareaHTMLAttributes<HTMLTextAreaElement>).rows ?? 3} />
      ) : (
        <input {...(rest as InputHTMLAttributes<HTMLInputElement>)} {...common} />
      )}
      {hint && (
        <p id={hintId} className="ds-field__hint">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="ds-field__error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
