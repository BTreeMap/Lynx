import React, { useCallback, useEffect, useLayoutEffect, useMemo, useState } from 'react';
import type { ReactNode } from 'react';
import { ThemeContext, type ThemeMode } from '../contexts/ThemeContext';

const STORAGE_KEY = 'lynx_theme';

const getSystemTheme = (): 'light' | 'dark' =>
    window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';

const readStoredMode = (): ThemeMode => {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === 'light' || stored === 'dark' || stored === 'system') {
        return stored;
    }
    return 'system';
};

const applyThemeClass = (theme: 'light' | 'dark') => {
    const root = document.documentElement;
    root.classList.toggle('dark', theme === 'dark');
    root.style.colorScheme = theme;
};

export const ThemeProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [mode, setModeState] = useState<ThemeMode>(() => readStoredMode());
    const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(() => getSystemTheme());

    const resolvedTheme: 'light' | 'dark' = mode === 'system' ? systemTheme : mode;

    // Keep the document class in sync with the resolved theme. A layout effect
    // ensures the class lands before passive effects in consumers read computed
    // styles (e.g. chart colors derived from CSS tokens).
    useLayoutEffect(() => {
        applyThemeClass(resolvedTheme);
    }, [resolvedTheme]);

    // Track OS preference changes while in "system" mode.
    useEffect(() => {
        const media = window.matchMedia('(prefers-color-scheme: dark)');
        const handler = (event: MediaQueryListEvent) => {
            setSystemTheme(event.matches ? 'dark' : 'light');
        };
        media.addEventListener('change', handler);
        return () => media.removeEventListener('change', handler);
    }, []);

    const setMode = useCallback((next: ThemeMode) => {
        setModeState(next);
        localStorage.setItem(STORAGE_KEY, next);
    }, []);

    const toggle = useCallback(() => {
        setModeState((current) => {
            const effective = current === 'system' ? getSystemTheme() : current;
            const next: ThemeMode = effective === 'dark' ? 'light' : 'dark';
            localStorage.setItem(STORAGE_KEY, next);
            return next;
        });
    }, []);

    const value = useMemo(
        () => ({ mode, resolvedTheme, setMode, toggle }),
        [mode, resolvedTheme, setMode, toggle],
    );

    return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
};
