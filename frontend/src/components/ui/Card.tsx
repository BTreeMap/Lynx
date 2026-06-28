import React from 'react';
import { cn } from '../../lib/cn';

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
    as?: 'div' | 'section' | 'article';
}

export const Card: React.FC<CardProps> = ({ as: Tag = 'div', className, children, ...props }) => (
    <Tag
        className={cn(
            'min-w-0 overflow-hidden rounded-2xl border border-border bg-surface shadow-soft',
            className,
        )}
        {...props}
    >
        {children}
    </Tag>
);

export const CardHeader: React.FC<React.HTMLAttributes<HTMLDivElement>> = ({
    className,
    children,
    ...props
}) => (
    <div className={cn('flex flex-col gap-1 p-4 sm:p-6', className)} {...props}>
        {children}
    </div>
);

export interface CardSectionHeaderProps extends React.HTMLAttributes<HTMLDivElement> {
    actions?: React.ReactNode;
}

export const CardSectionHeader: React.FC<CardSectionHeaderProps> = ({
    actions,
    className,
    children,
    ...props
}) => (
    <CardHeader
        className={cn(
            'border-b border-border/70 bg-surface/70',
            actions && 'flex-col items-stretch gap-4 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between sm:gap-3',
            className,
        )}
        {...props}
    >
        <div className="min-w-0">{children}</div>
        {actions && <div className="min-w-0">{actions}</div>}
    </CardHeader>
);

export const CardTitle: React.FC<React.HTMLAttributes<HTMLHeadingElement>> = ({
    className,
    children,
    ...props
}) => (
    <h2 className={cn('text-base font-semibold tracking-tight text-fg sm:text-lg', className)} {...props}>
        {children}
    </h2>
);

export const CardDescription: React.FC<React.HTMLAttributes<HTMLParagraphElement>> = ({
    className,
    children,
    ...props
}) => (
    <p className={cn('text-sm text-fg-muted', className)} {...props}>
        {children}
    </p>
);

export const CardBody: React.FC<React.HTMLAttributes<HTMLDivElement>> = ({
    className,
    children,
    ...props
}) => (
    <div className={cn('p-4 pt-0 sm:p-6 sm:pt-0', className)} {...props}>
        {children}
    </div>
);
