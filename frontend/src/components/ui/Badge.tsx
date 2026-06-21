import React from 'react';
import { cn } from '../../lib/cn';

export type BadgeTone = 'neutral' | 'primary' | 'success' | 'danger' | 'accent';

const tones: Record<BadgeTone, string> = {
    neutral: 'bg-surface-2 text-fg-muted border-border',
    primary: 'bg-primary-soft text-primary-soft-fg border-primary/30',
    success: 'bg-success-soft text-success-soft-fg border-success/30',
    danger: 'bg-danger-soft text-danger-soft-fg border-danger/30',
    accent: 'bg-accent/15 text-accent border-accent/30',
};

export interface BadgeProps extends React.HTMLAttributes<HTMLSpanElement> {
    tone?: BadgeTone;
    dot?: boolean;
}

export const Badge: React.FC<BadgeProps> = ({
    tone = 'neutral',
    dot = false,
    className,
    children,
    ...props
}) => (
    <span
        className={cn(
            'inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium',
            tones[tone],
            className,
        )}
        {...props}
    >
        {dot && <span className="h-1.5 w-1.5 rounded-full bg-current" aria-hidden />}
        {children}
    </span>
);
