import type { PinnedSide } from '../ui/tableStyles';

/**
 * The links table's column model.
 *
 * This list is the single source of truth for three facts that used to be
 * maintained independently in JSX — the header labels, the declared widths, and the
 * column *count* used for the virtualiser's spacer `colSpan`. Keeping them in one
 * array means a column cannot be added to the header without also getting a width,
 * and `colSpan` can no longer disagree with the number of rendered columns.
 *
 * Widths are declared in pixels and applied through a `<colgroup>` under
 * `table-fixed`, so cell *content* can never dictate layout. That is the fix for the
 * original defect: an unconstrained principal column (a 36-character UUID marked
 * `whitespace-nowrap`) pushed the table past its container, and the trailing actions
 * scrolled out of sight with no indication they existed.
 */
export interface LinkColumn {
    readonly key: string;
    readonly header: string;
    /** Declared width in px, or `'flex'` to absorb the leftover space. */
    readonly width: number | 'flex';
    readonly align?: 'end';
    /** Rendered only for admins, who alone can see the owning principal. */
    readonly adminOnly?: boolean;
    /** Pinned against the scrollport so it survives horizontal scrolling. */
    readonly pin?: PinnedSide;
}

/**
 * Floor for the flexible column. Below this, a destination URL truncates to the
 * point of being unidentifiable, so the table scrolls horizontally instead — with
 * identity and actions pinned at either edge.
 */
const MIN_FLEX_WIDTH = 240;

/*
  Widths are sized to the content each cell actually renders, plus the 32px of cell
  padding `TD` applies at `sm:` and above. A column narrower than its content would
  truncate every row, which is worse than scrolling — so the flexible destination
  column is the only one expected to truncate, and it is the one whose full value is
  least often needed at a glance.
*/
export const LINK_COLUMNS: readonly LinkColumn[] = [
    // ~13 monospace characters; longer custom codes truncate and carry a tooltip.
    { key: 'shortCode', header: 'Short code', width: 144, pin: 'left' },
    { key: 'destination', header: 'Destination', width: 'flex' },
    { key: 'clicks', header: 'Clicks', width: 88, align: 'end' },
    // Wide enough for the longer "Inactive" badge, dot and padding included.
    { key: 'status', header: 'Status', width: 112 },
    // Fits a seconds-less numeric locale date-time in every locale checked, from
    // "28/07/2026, 14:12" (17 glyphs) up to ko-KR's "2026. 01. 28. 09:12" (19).
    { key: 'created', header: 'Created', width: 176 },
    // Fits an abbreviated principal, e.g. "53810659…8f27".
    { key: 'createdBy', header: 'Created by', width: 132, adminOnly: true },
    // Three 32px controls plus their gaps and the cell's own padding.
    { key: 'actions', header: 'Actions', width: 140, align: 'end', pin: 'right' },
];

export const visibleLinkColumns = (isAdmin: boolean): readonly LinkColumn[] =>
    isAdmin ? LINK_COLUMNS : LINK_COLUMNS.filter((column) => !column.adminOnly);

/**
 * Narrowest width at which every column still honours its declared size.
 *
 * A fold over the column widths: identity `0`, associative `+`, with the flexible
 * column contributing its floor. Derived rather than hard-coded so the admin and
 * non-admin tables cannot drift out of step with the column list above.
 */
export const minTableWidth = (columns: readonly LinkColumn[]): number =>
    columns.reduce(
        (total, column) => total + (column.width === 'flex' ? MIN_FLEX_WIDTH : column.width),
        0,
    );
