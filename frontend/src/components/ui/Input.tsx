import React from 'react';
import { cn } from '../../lib/cn';

const fieldBase =
    'w-full rounded-lg border border-border bg-bg text-fg placeholder:text-fg-subtle ' +
    'transition-[border-color,box-shadow] duration-150 ' +
    'focus:border-primary focus-visible:outline-none focus:ring-2 focus:ring-ring/30 ' +
    'disabled:cursor-not-allowed disabled:opacity-60';

export type InputProps = React.InputHTMLAttributes<HTMLInputElement> & {
    invalid?: boolean;
};

export const Input = React.forwardRef<HTMLInputElement, InputProps>(
    ({ className, invalid, ...props }, ref) => (
        <input
            ref={ref}
            className={cn(
                fieldBase,
                'h-10 px-3 text-[13px] sm:h-11 sm:px-3.5 sm:text-sm',
                invalid && 'border-danger focus:border-danger focus:ring-danger/30',
                className,
            )}
            {...props}
        />
    ),
);

Input.displayName = 'Input';

export type SelectProps = React.SelectHTMLAttributes<HTMLSelectElement>;

export const Select = React.forwardRef<HTMLSelectElement, SelectProps>(
    ({ className, children, ...props }, ref) => (
        <select
            ref={ref}
            className={cn(
                fieldBase,
                'h-10 px-3 text-[13px] appearance-none cursor-pointer sm:h-11 sm:px-3.5 sm:text-sm',
                className,
            )}
            {...props}
        >
            {children}
        </select>
    ),
);

Select.displayName = 'Select';

export interface FieldProps {
    label: string;
    htmlFor?: string;
    hint?: string;
    required?: boolean;
    className?: string;
    children: React.ReactNode;
}

/** Labelled form field wrapper. */
export const Field: React.FC<FieldProps> = ({
    label,
    htmlFor,
    hint,
    required,
    className,
    children,
}) => (
    <div className={cn('flex flex-col gap-1.5 sm:gap-2', className)}>
        <label htmlFor={htmlFor} className="text-[13px] font-medium text-fg sm:text-sm">
            {label}
            {required && <span className="ml-1 text-danger">*</span>}
        </label>
        {children}
        {hint && <p className="text-[13px] text-fg-subtle">{hint}</p>}
    </div>
);
