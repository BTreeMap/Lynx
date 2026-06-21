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
            'flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border bg-surface-2/40 px-6 py-12 text-center',
            className,
        )}
    >
        {icon && (
            <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary-soft text-primary-soft-fg">
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
