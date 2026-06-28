import React from 'react';
import { cn } from '../../lib/cn';
import { Spinner } from './Spinner';

export type ButtonVariant =
    | 'primary'
    | 'secondary'
    | 'outline'
    | 'ghost'
    | 'danger'
    | 'success';

export type ButtonSize = 'sm' | 'md' | 'lg';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
    variant?: ButtonVariant;
    size?: ButtonSize;
    isLoading?: boolean;
    leftIcon?: React.ReactNode;
    rightIcon?: React.ReactNode;
    fullWidth?: boolean;
}

const base =
    'inline-flex items-center justify-center gap-2 rounded-lg font-medium whitespace-nowrap ' +
    'transition-all duration-150 ' +
    'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring ' +
    'disabled:cursor-not-allowed disabled:opacity-55 disabled:pointer-events-none ' +
    'active:translate-y-px select-none cursor-pointer';

const variants: Record<ButtonVariant, string> = {
    primary:
        'bg-primary text-primary-fg shadow-soft hover:bg-primary-hover active:bg-primary-active',
    secondary:
        'bg-surface-2 text-fg border border-border hover:border-border-strong hover:bg-elevated',
    outline:
        'bg-transparent text-fg border border-border hover:border-primary hover:text-primary',
    ghost: 'bg-transparent text-fg-muted hover:bg-surface-2 hover:text-fg',
    danger:
        'bg-transparent text-danger border border-danger/60 hover:bg-danger hover:text-danger-fg',
    success:
        'bg-success text-success-fg shadow-soft hover:opacity-90',
};

const sizes: Record<ButtonSize, string> = {
    sm: 'h-8 px-2.5 text-xs sm:px-3',
    md: 'h-9 px-3.5 text-sm sm:h-10 sm:px-4',
    lg: 'h-11 px-5 text-sm sm:h-12 sm:px-6 sm:text-base',
};

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
    (
        {
            variant = 'primary',
            size = 'md',
            isLoading = false,
            leftIcon,
            rightIcon,
            fullWidth,
            className,
            children,
            disabled,
            ...props
        },
        ref,
    ) => (
        <button
            ref={ref}
            disabled={disabled || isLoading}
            className={cn(base, variants[variant], sizes[size], fullWidth && 'w-full', className)}
            {...props}
        >
            {isLoading ? <Spinner /> : leftIcon}
            {children}
            {!isLoading && rightIcon}
        </button>
    ),
);

Button.displayName = 'Button';
