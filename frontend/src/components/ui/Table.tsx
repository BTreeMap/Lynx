import React from 'react';
import { cn } from '../../lib/cn';

export const TableScroll: React.FC<React.HTMLAttributes<HTMLDivElement>> = ({
    className,
    children,
    ...props
}) => (
    <div
        className={cn(
            'overflow-x-auto rounded-2xl border border-border bg-surface shadow-soft',
            className,
        )}
        {...props}
    >
        {children}
    </div>
);

export const Table: React.FC<React.TableHTMLAttributes<HTMLTableElement>> = ({
    className,
    children,
    ...props
}) => (
    <table className={cn('w-full border-collapse text-left', className)} {...props}>
        {children}
    </table>
);

export const THead: React.FC<React.HTMLAttributes<HTMLTableSectionElement>> = ({
    className,
    children,
    ...props
}) => (
    <thead
        className={cn('border-b border-border bg-surface-2/60', className)}
        {...props}
    >
        {children}
    </thead>
);

export const TBody: React.FC<React.HTMLAttributes<HTMLTableSectionElement>> = ({
    className,
    children,
    ...props
}) => (
    <tbody className={className} {...props}>
        {children}
    </tbody>
);

export const TR: React.FC<React.HTMLAttributes<HTMLTableRowElement>> = ({
    className,
    children,
    ...props
}) => (
    <tr
        className={cn('border-b border-border/60 last:border-0', className)}
        {...props}
    >
        {children}
    </tr>
);

export const TH: React.FC<React.ThHTMLAttributes<HTMLTableCellElement>> = ({
    className,
    children,
    ...props
}) => (
    <th
        className={cn(
            'px-3 py-2.5 text-xs font-semibold uppercase tracking-wide text-fg-subtle whitespace-nowrap sm:px-4 sm:py-3',
            className,
        )}
        {...props}
    >
        {children}
    </th>
);

export const TD: React.FC<React.TdHTMLAttributes<HTMLTableCellElement>> = ({
    className,
    children,
    ...props
}) => (
    <td
        className={cn(
            'px-3 py-2.5 text-sm text-fg align-middle sm:px-4 sm:py-3',
            className,
        )}
        {...props}
    >
        {children}
    </td>
);
