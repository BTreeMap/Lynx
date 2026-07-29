/**
 * Chart colours, referenced rather than read.
 *
 * Recharts writes these straight onto SVG presentation attributes (`fill`), which are
 * CSS properties — so a `var(--token)` reference resolves in the browser, against
 * whichever theme is currently applied. That makes `index.css` the single source of
 * truth in the strongest sense: no value is copied into JavaScript at all.
 *
 * This replaces a `getComputedStyle` sample held in component state and re-taken from an
 * effect on every theme change. That version had to re-render the charts to recolour
 * them, and was correct only because the theme class happened to be written in a layout
 * effect that ran first — an ordering dependency between two unrelated modules. With
 * variables there is nothing to synchronise: the browser recolours the existing nodes.
 */

const SERIES: readonly string[] = [
    'var(--chart-1)',
    'var(--chart-2)',
    'var(--chart-3)',
    'var(--chart-4)',
    'var(--chart-5)',
    'var(--chart-6)',
    'var(--chart-7)',
    'var(--chart-8)',
    'var(--chart-9)',
    'var(--chart-10)',
];

/** The remainder slice: deliberately outside the series palette. */
const OTHER = 'var(--chart-other)';

export const CHART_AXIS_COLOR = 'var(--fg-subtle)';
export const CHART_CURSOR_COLOR = 'var(--chart-cursor)';

/** Colour for slice *i*, wrapping the series; the remainder has its own muted tone. */
export const sliceColor = (index: number, isOther: boolean): string =>
    isOther ? OTHER : SERIES[index % SERIES.length];
