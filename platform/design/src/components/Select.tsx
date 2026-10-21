import { useId } from "react";
import type { Ref, SelectHTMLAttributes } from "react";
import { cx } from "../cx";

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export type SelectProps = Omit<SelectHTMLAttributes<HTMLSelectElement>, "className" | "children"> & {
  label: string;
  hideLabel?: boolean;
  options: SelectOption[];
  /** Adds a first, disabled option with this text and an empty value. */
  placeholder?: string;
  hint?: string;
  error?: string;
  ref?: Ref<HTMLSelectElement>;
};

/** Native select with a themed look: works on every phone, keyboard and screen reader. */
export function Select({ label, hideLabel, options, placeholder, hint, error, id: idProp, ...rest }: SelectProps) {
  const auto = useId();
  const id = idProp ?? auto;
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy = [error && errorId, hint && hintId].filter(Boolean).join(" ") || undefined;
  return (
    <div className={cx("ds-field", error && "ds-field--invalid")}>
      <label htmlFor={id} className={cx("ds-field__label", hideLabel && "ds-sr-only")}>
        {label}
      </label>
      <span className="ds-select">
        <select {...rest} id={id} className="ds-input ds-select__control" aria-invalid={error ? true : undefined} aria-describedby={describedBy}>
          {placeholder !== undefined && (
            <option value="" disabled>
              {placeholder}
            </option>
          )}
          {options.map((o) => (
            <option key={o.value} value={o.value} disabled={o.disabled}>
              {o.label}
            </option>
          ))}
        </select>
      </span>
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
