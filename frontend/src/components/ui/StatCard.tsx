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
            'flex w-full min-w-0 items-center gap-3 rounded-2xl border border-border bg-surface p-3.5 shadow-soft sm:gap-4 sm:p-5',
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
        <div className="min-w-0 flex-1">
            <p className="text-[11px] font-medium uppercase tracking-wide text-fg-subtle sm:text-xs">{label}</p>
            <div className="mt-0.5 break-words text-lg font-semibold tracking-tight text-fg sm:mt-1 sm:text-2xl">
                {value}
            </div>
            {hint && <p className="text-[11px] text-fg-muted sm:text-xs">{hint}</p>}
        </div>
    </div>
);
