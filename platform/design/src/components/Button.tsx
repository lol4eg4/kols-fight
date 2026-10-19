import { useId } from "react";
import type { ButtonHTMLAttributes, MouseEvent, Ref } from "react";
import { cx } from "../cx";

export type ButtonVariant = "primary" | "fight" | "secondary";

/** Class names of a button, for a link that must look like one. */
export function buttonClass(variant: ButtonVariant = "primary", extra?: string): string {
  return cx("ds-btn", `ds-btn--${variant}`, extra);
}

type Disabled = { disabled?: false; reason?: undefined } | { disabled: true; reason: string };

export type ButtonProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "disabled" | "className"> &
  Disabled & {
    variant?: ButtonVariant;
    /** Full width. */
    block?: boolean;
    ref?: Ref<HTMLButtonElement>;
  };

/**
 * Pixel button with a hard offset shadow. A disabled button stays focusable (aria-disabled, no click)
 * and must say why in visible text next to it.
 */
export function Button({ variant = "primary", block, disabled, reason, onClick, type = "button", children, ref, ...rest }: ButtonProps) {
  const reasonId = useId();
  const handleClick = (e: MouseEvent<HTMLButtonElement>) => {
    if (disabled) {
      e.preventDefault();
      return;
    }
    onClick?.(e);
  };
  return (
    <span className={cx("ds-btn-wrap", block && "ds-btn-wrap--block")}>
      <button
        {...rest}
        ref={ref}
        type={type}
        className={buttonClass(variant, block ? "ds-btn--block" : undefined)}
        aria-disabled={disabled || undefined}
        aria-describedby={disabled ? reasonId : rest["aria-describedby"]}
        onClick={handleClick}
      >
        {children}
      </button>
      {disabled && (
        <span id={reasonId} className="ds-btn-reason">
          {reason}
        </span>
      )}
    </span>
  );
}
