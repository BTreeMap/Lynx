import React from 'react';
import { cn } from '../../lib/cn';

export interface PageShellProps extends React.HTMLAttributes<HTMLElement> {
    as?: 'main' | 'section' | 'div';
}

export const PageShell: React.FC<PageShellProps> = ({
    as: Tag = 'main',
    className,
    children,
    ...props
}) => (
    <Tag
        className={cn(
            'mx-auto max-w-6xl space-y-6 px-3 py-6 sm:space-y-8 sm:px-6 sm:py-10',
            className,
        )}
        {...props}
    >
        {children}
    </Tag>
);

export interface PageIntroProps extends Omit<React.HTMLAttributes<HTMLElement>, 'title'> {
    title: React.ReactNode;
    description?: React.ReactNode;
    actions?: React.ReactNode;
}

export const PageIntro: React.FC<PageIntroProps> = ({
    title,
    description,
    actions,
    className,
    ...props
}) => (
    <section className={cn('space-y-3 sm:space-y-4', className)} {...props}>
        {actions}
        <div className="space-y-1.5">
            <h1 className="text-xl font-bold tracking-tight text-fg sm:text-3xl">{title}</h1>
            {description && <p className="max-w-2xl text-sm text-fg-muted">{description}</p>}
        </div>
    </section>
);
