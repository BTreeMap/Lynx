import React from 'react';
import { Spinner } from './Spinner';
import {
    iconButtonClass,
    type IconButtonStyleOptions,
} from './iconButtonStyles';

export interface IconButtonProps
    extends React.ButtonHTMLAttributes<HTMLButtonElement>,
        Omit<IconButtonStyleOptions, 'className'> {
    label: string;
    isLoading?: boolean;
}

export const IconButton = React.forwardRef<HTMLButtonElement, IconButtonProps>(
    (
        {
            size = 'md',
            variant = 'ghost',
            label,
            isLoading = false,
            className,
            children,
            disabled,
            ...props
        },
        ref,
    ) => (
        <button
            ref={ref}
            type="button"
            aria-label={label}
            title={label}
            disabled={disabled || isLoading}
            className={iconButtonClass({ size, variant, className })}
            {...props}
        >
            {isLoading ? <Spinner /> : children}
        </button>
    ),
);

IconButton.displayName = 'IconButton';
