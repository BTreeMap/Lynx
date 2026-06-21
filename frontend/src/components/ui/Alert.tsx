import React from 'react';
import { AlertCircle, CheckCircle2, Info } from 'lucide-react';
import { cn } from '../../lib/cn';

export type AlertTone = 'error' | 'success' | 'info';

const config: Record<AlertTone, { icon: React.ReactNode; classes: string }> = {
    error: {
        icon: <AlertCircle className="h-4.5 w-4.5" />,
        classes: 'bg-danger-soft text-danger-soft-fg border-danger/30',
    },
    success: {
        icon: <CheckCircle2 className="h-4.5 w-4.5" />,
        classes: 'bg-success-soft text-success-soft-fg border-success/30',
    },
    info: {
        icon: <Info className="h-4.5 w-4.5" />,
        classes: 'bg-primary-soft text-primary-soft-fg border-primary/30',
    },
};

export interface AlertProps {
    tone?: AlertTone;
    children: React.ReactNode;
    className?: string;
}

export const Alert: React.FC<AlertProps> = ({ tone = 'error', children, className }) => {
    const { icon, classes } = config[tone];
    return (
        <div
            role="alert"
            className={cn(
                'flex items-start gap-2.5 rounded-xl border px-4 py-3 text-sm',
                classes,
                className,
            )}
        >
            <span className="mt-px shrink-0">{icon}</span>
            <span className="min-w-0">{children}</span>
        </div>
    );
};
