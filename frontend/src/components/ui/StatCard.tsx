import React from 'react';
import { cn } from '../../lib/cn';

export interface StatCardProps {
    label: string;
    value: React.ReactNode;
    icon?: React.ReactNode;
    hint?: React.ReactNode;
    tone?: 'primary' | 'success' | 'accent' | 'neutral';
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
    className,
}) => (
    <div
        className={cn(
            'flex w-full min-w-0 items-center gap-4 rounded-2xl border border-border bg-surface p-4 shadow-soft sm:p-5',
            className,
        )}
    >
        {icon && (
            <div
                className={cn(
                    'flex h-11 w-11 shrink-0 items-center justify-center rounded-xl',
                    iconTones[tone],
                )}
            >
                {icon}
            </div>
        )}
        <div className="min-w-0 flex-1">
            <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">{label}</p>
            <div className="mt-1 break-words text-xl font-semibold tracking-tight text-fg sm:text-2xl">
                {value}
            </div>
            {hint && <p className="text-xs text-fg-muted">{hint}</p>}
        </div>
    </div>
);
