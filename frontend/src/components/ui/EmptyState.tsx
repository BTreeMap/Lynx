import React from 'react';
import { cn } from '../../lib/cn';

export interface EmptyStateProps {
    icon?: React.ReactNode;
    title: string;
    description?: string;
    action?: React.ReactNode;
    className?: string;
}

export const EmptyState: React.FC<EmptyStateProps> = ({
    icon,
    title,
    description,
    action,
    className,
}) => (
    <div
        className={cn(
            'flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border bg-surface-2/40 px-4 py-8 text-center sm:px-6 sm:py-12',
            className,
        )}
    >
        {icon && (
            <div className="flex h-10 w-10 items-center justify-center rounded-full bg-primary-soft text-primary-soft-fg sm:h-12 sm:w-12">
                {icon}
            </div>
        )}
        <div className="space-y-1">
            <p className="text-sm font-semibold text-fg">{title}</p>
            {description && <p className="mx-auto max-w-sm text-sm text-fg-muted">{description}</p>}
        </div>
        {action}
    </div>
);
