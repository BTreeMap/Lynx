import React from 'react';
import { cn } from '../../lib/cn';

export interface StatCardProps {
    label: string;
    value: React.ReactNode;
    icon?: React.ReactNode;
    hint?: React.ReactNode;
    tone?: 'primary' | 'success' | 'accent' | 'neutral';
    valueClassName?: string;
    className?: string;
}

const iconTones = {
    primary: 'bg-primary-soft text-primary-soft-fg',
    success: 'bg-success-soft text-success-soft-fg',
    accent: 'bg-accent/15 text-accent',
    neutral: 'bg-surface-2 text-fg-muted',
};

export const StatCard: React.FC<StatCardProps> = ({
    label,
    value,
    icon,
    hint,
    tone = 'neutral',
    valueClassName,
    className,
}) => (
    <div
        className={cn(
            'grid min-h-full w-full min-w-0 grid-cols-[auto_minmax(0,1fr)] items-start gap-3 rounded-2xl border border-border bg-surface p-3.5 shadow-soft sm:gap-4 sm:p-5',
            className,
        )}
    >
        {icon && (
            <div
                className={cn(
                    'flex h-10 w-10 shrink-0 items-center justify-center rounded-xl sm:h-11 sm:w-11',
                    iconTones[tone],
                )}
            >
                {icon}
            </div>
        )}
        <div className="min-w-0 space-y-1.5">
            <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">{label}</p>
            <div
                className={cn(
                    'break-words text-lg font-semibold leading-tight tracking-tight text-fg sm:text-2xl',
                    valueClassName,
                )}
            >
                {value}
            </div>
            {hint && <p className="text-xs text-fg-muted">{hint}</p>}
        </div>
    </div>
);
