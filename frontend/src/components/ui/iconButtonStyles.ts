import { cn } from '../../lib/cn';

export type IconButtonSize = 'sm' | 'md';
export type IconButtonVariant = 'ghost' | 'outline' | 'danger' | 'success';

/**
 * `Record` over the closed size/variant unions rather than a lookup with a fallback:
 * adding a variant without styling it is a compile error, not a silently unstyled
 * control.
 */
const sizes: Record<IconButtonSize, string> = {
    sm: 'h-8 w-8',
    md: 'h-10 w-10',
};

const variants: Record<IconButtonVariant, string> = {
    ghost: 'text-fg-muted hover:bg-surface-2 hover:text-fg',
    outline: 'border border-border text-fg-muted hover:border-border-strong hover:text-fg',
    danger: 'text-danger hover:bg-danger hover:text-danger-fg',
    success: 'text-success hover:bg-success hover:text-success-fg',
};

const base =
    'inline-flex shrink-0 items-center justify-center rounded-lg transition-colors duration-150 cursor-pointer ' +
    'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring ' +
    'disabled:cursor-not-allowed disabled:opacity-55';

export interface IconButtonStyleOptions {
    size?: IconButtonSize;
    variant?: IconButtonVariant;
    className?: string;
}

/**
 * The single definition of icon-control styling. Lives outside `IconButton.tsx` so
 * that non-`<button>` controls (router `<Link>`, anchors) can render identically
 * instead of re-deriving the classes — previously each such link carried its own
 * hand-copied 200-character class string. Callers that are not buttons must supply
 * their own `aria-label`/`title`.
 */
export const iconButtonClass = ({
    size = 'md',
    variant = 'ghost',
    className,
}: IconButtonStyleOptions = {}): string =>
    cn(base, sizes[size], variants[variant], className);
