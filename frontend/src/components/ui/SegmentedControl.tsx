import React from 'react';
import { cn } from '../../lib/cn';

export interface SegmentOption<T extends string> {
    value: T;
    label: string;
    icon?: React.ReactNode;
}

export interface SegmentedControlProps<T extends string> {
    options: SegmentOption<T>[];
    value: T;
    onChange: (value: T) => void;
    ariaLabel?: string;
    className?: string;
}

/** Accessible segmented (tab-like) control built on radio semantics. */
export function SegmentedControl<T extends string>({
    options,
    value,
    onChange,
    ariaLabel,
    className,
}: SegmentedControlProps<T>) {
    return (
        <div
            role="tablist"
            aria-label={ariaLabel}
            className={cn(
                'inline-flex flex-wrap items-center gap-1 rounded-xl border border-border bg-surface-2 p-0.5 sm:p-1',
                className,
            )}
        >
            {options.map((option) => {
                const active = option.value === value;
                return (
                    <button
                        key={option.value}
                        type="button"
                        role="tab"
                        aria-selected={active}
                        onClick={() => onChange(option.value)}
                        className={cn(
                            'inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium transition-colors duration-150 cursor-pointer sm:px-3 sm:text-[13px]',
                            'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                            active
                                ? 'bg-surface text-fg shadow-soft'
                                : 'text-fg-muted hover:text-fg',
                        )}
                    >
                        {option.icon}
                        {option.label}
                    </button>
                );
            })}
        </div>
    );
}
