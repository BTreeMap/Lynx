import React from 'react';
import { Link } from 'react-router-dom';
import { cn } from '../../lib/cn';

export interface LogoProps {
    className?: string;
    wordmarkClassName?: string;
    /** Render as a link to the dashboard root. */
    asLink?: boolean;
}

const Mark = () => (
    <span className="relative flex h-9 w-9 items-center justify-center rounded-xl bg-baltic-blue-500 shadow-soft">
        <svg viewBox="0 0 24 24" className="h-5 w-5 text-white" fill="none" aria-hidden>
            <path
                d="M9.5 13.5 5.8 17.2a3.3 3.3 0 0 1-4.7-4.7l3.2-3.2a3.3 3.3 0 0 1 4.7 0"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
            />
            <path
                d="M14.5 10.5 18.2 6.8a3.3 3.3 0 0 1 4.7 4.7l-3.2 3.2a3.3 3.3 0 0 1-4.7 0"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
            />
            <path
                d="m9 15 6-6"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
            />
        </svg>
    </span>
);

const Wordmark: React.FC<{ className?: string }> = ({ className }) => (
    <span className={cn('text-xl font-bold tracking-tight text-fg', className)}>
        Lynx
    </span>
);

export const Logo: React.FC<LogoProps> = ({ className, wordmarkClassName, asLink = true }) => {
    const content = (
        <span className={cn('inline-flex items-center gap-2.5', className)}>
            <Mark />
            <Wordmark className={wordmarkClassName} />
        </span>
    );

    if (asLink) {
        return (
            <Link
                to="/"
                className="inline-flex items-center rounded-lg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
                aria-label="Lynx home"
            >
                {content}
            </Link>
        );
    }
    return content;
};
