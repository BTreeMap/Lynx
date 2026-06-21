import React from 'react';
import { cn } from '../../lib/cn';

export interface IconButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
    size?: 'sm' | 'md';
    variant?: 'ghost' | 'outline';
    label: string;
}

const sizes = {
    sm: 'h-8 w-8',
    md: 'h-10 w-10',
};

const variants = {
    ghost: 'text-fg-muted hover:bg-surface-2 hover:text-fg',
    outline: 'border border-border text-fg-muted hover:border-border-strong hover:text-fg',
};

export const IconButton = React.forwardRef<HTMLButtonElement, IconButtonProps>(
    ({ size = 'md', variant = 'ghost', label, className, children, ...props }, ref) => (
        <button
            ref={ref}
            type="button"
            aria-label={label}
            title={label}
            className={cn(
                'inline-flex items-center justify-center rounded-lg transition-colors duration-150 cursor-pointer',
                'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                'disabled:cursor-not-allowed disabled:opacity-55',
                sizes[size],
                variants[variant],
                className,
            )}
            {...props}
        >
            {children}
        </button>
    ),
);

IconButton.displayName = 'IconButton';
