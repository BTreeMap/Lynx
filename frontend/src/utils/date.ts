/** Format a Unix timestamp (seconds) as a locale-aware date-time string. */
export const formatDate = (timestamp: number): string =>
    new Date(timestamp * 1000).toLocaleString();

/**
 * Built once at module load rather than per call: constructing an
 * `Intl.DateTimeFormat` is orders of magnitude more expensive than formatting with
 * one, and the links table formats a date for every visible row on every render.
 * Consequence: the locale is resolved at load time, which is sound only because the
 * app has no runtime locale switch.
 */
const compactDateTime = new Intl.DateTimeFormat(undefined, {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    // All-numeric fields and a 24-hour clock keep the rendered width bounded at ~17
    // characters in every locale, which is what lets a fixed-width column hold it
    // without truncating. A named month ("Jan 28, 2026") or a 12-hour clock with its
    // AM/PM suffix reaches 22 and overflows. The locale still decides field order and
    // separators; only the field widths are pinned.
    hour12: false,
});

/**
 * Bounded-width date for list rows — the same fields as `formatDate` minus seconds,
 * which are never load-bearing when scanning a table and cost a quarter of the column.
 * Pair it with `formatDate` in a `title` attribute wherever full precision must stay
 * recoverable.
 */
export const formatDateCompact = (timestamp: number): string =>
    compactDateTime.format(new Date(timestamp * 1000));
