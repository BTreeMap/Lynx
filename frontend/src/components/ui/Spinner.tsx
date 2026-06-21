import React from 'react';
import { cn } from '../../lib/cn';

interface SpinnerProps {
    className?: string;
}

/** Minimal accessible loading spinner. */
export const Spinner: React.FC<SpinnerProps> = ({ className }) => (
    <svg
        className={cn('h-4 w-4 animate-spin', className)}
        viewBox="0 0 24 24"
        fill="none"
        role="status"
        aria-label="Loading"
    >
        <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
        <path
            className="opacity-90"
            fill="currentColor"
            d="M4 12a8 8 0 0 1 8-8V0C5.373 0 0 5.373 0 12h4z"
        />
    </svg>
);
