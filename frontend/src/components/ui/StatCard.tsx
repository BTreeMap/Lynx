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
            'flex items-center gap-4 rounded-2xl border border-border bg-surface p-4 shadow-soft sm:p-5',
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
        <div className="min-w-0">
            <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">{label}</p>
            <p className="mt-0.5 truncate text-2xl font-semibold tracking-tight text-fg">{value}</p>
            {hint && <p className="text-xs text-fg-muted">{hint}</p>}
        </div>
    </div>
);
