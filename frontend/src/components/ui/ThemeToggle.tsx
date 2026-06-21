import React from 'react';
import { Monitor, Moon, Sun } from 'lucide-react';
import { useTheme } from '../../hooks/useTheme';
import type { ThemeMode } from '../../contexts/ThemeContext';
import { cn } from '../../lib/cn';

const options: { value: ThemeMode; label: string; icon: React.ReactNode }[] = [
    { value: 'light', label: 'Light', icon: <Sun className="h-4 w-4" /> },
    { value: 'system', label: 'System', icon: <Monitor className="h-4 w-4" /> },
    { value: 'dark', label: 'Dark', icon: <Moon className="h-4 w-4" /> },
];

/** Three-way light / system / dark theme switch. */
export const ThemeToggle: React.FC = () => {
    const { mode, setMode } = useTheme();

    return (
        <div
            role="radiogroup"
            aria-label="Color theme"
            className="inline-flex items-center gap-0.5 rounded-lg border border-border bg-surface-2 p-0.5"
        >
            {options.map((option) => {
                const active = mode === option.value;
                return (
                    <button
                        key={option.value}
                        type="button"
                        role="radio"
                        aria-checked={active}
                        aria-label={option.label}
                        title={`${option.label} theme`}
                        onClick={() => setMode(option.value)}
                        className={cn(
                            'inline-flex h-8 w-8 items-center justify-center rounded-md transition-colors duration-150 cursor-pointer',
                            'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                            active ? 'bg-surface text-primary shadow-soft' : 'text-fg-subtle hover:text-fg',
                        )}
                    >
                        {option.icon}
                    </button>
                );
            })}
        </div>
    );
};
