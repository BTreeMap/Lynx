import { createContext } from 'react';

export type ThemeMode = 'light' | 'dark' | 'system';

export interface ThemeContextType {
    /** User-selected mode (light, dark, or follow system). */
    mode: ThemeMode;
    /** The effective theme actually applied to the document. */
    resolvedTheme: 'light' | 'dark';
    setMode: (mode: ThemeMode) => void;
    /** Cycle between light and dark (resolving system first). */
    toggle: () => void;
}

export const ThemeContext = createContext<ThemeContextType | undefined>(undefined);
