import React from 'react';
import { cn } from '../../lib/cn';

export const Skeleton: React.FC<React.HTMLAttributes<HTMLDivElement>> = ({
    className,
    ...props
}) => (
    <div
        className={cn('animate-skeleton rounded-lg bg-surface-2', className)}
        {...props}
    />
);
