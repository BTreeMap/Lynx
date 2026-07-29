/**
 * Date rendering.
 *
 * Every formatter here is built once at module load. Constructing an
 * `Intl.DateTimeFormat` is orders of magnitude more expensive than formatting with one,
 * and these run per row, per render — the links table formats a date for every visible
 * row, the analytics table for every bucket. The consequence is that the locale is
 * resolved at load time, which is sound only because the app has no runtime locale
 * switch.
 */

/**
 * `Date.prototype.toLocaleString`'s exact default field set — all six components,
 * numeric — reproduced so the cached formatter renders what it always has. A
 * `dateStyle`/`timeStyle` pair is *not* the same thing: it abbreviates the year.
 */
const fullDateTime = new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
    hour: 'numeric',
    minute: 'numeric',
    second: 'numeric',
});

/** Format a Unix timestamp (seconds) as a locale-aware date-time string. */
export const formatDate = (timestamp: number): string => fullDateTime.format(timestamp * 1000);

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
    compactDateTime.format(timestamp * 1000);

/*
  The analytics bucket keeps date and time as two formatters rather than one combined
  pattern: a single `DateTimeFormat` carrying both joins them with a locale separator
  ("28/07/2026, 14:12"), and the space-joined form below is what the activity table has
  always rendered.
*/
const bucketDate = new Intl.DateTimeFormat();
const bucketTime = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

/** Analytics bucket start (Unix seconds) as `date time`, without seconds. */
export const formatTimeBucket = (timestamp: number): string => {
    const at = timestamp * 1000;
    return `${bucketDate.format(at)} ${bucketTime.format(at)}`;
};
