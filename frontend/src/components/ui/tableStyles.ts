import { cn } from '../../lib/cn';

/**
 * Pinned-column styling for tables rendered inside a `TableScroll`.
 *
 * A pinned cell sticks to the horizontal scrollport, so it is painted *over* the
 * cells that scroll beneath it and must be opaque. The backgrounds below are only
 * correct in relation to the defaults in `Table.tsx`: `stickyHeaderCell` reproduces
 * `THead`'s solid `bg-surface-2`, and `stickyBodyCell` sits on `bg-surface` and
 * re-applies `ROW_HOVER` itself — which requires `group` on the `<tr>`, because a
 * pinned cell cannot let the row's own background show through. Change one of these
 * and the other three stop matching.
 */

export type PinnedSide = 'left' | 'right';

/**
 * Position plus the divider that makes the pin legible as a boundary.
 *
 * The divider is a box-shadow, not a border: under `border-collapse: collapse` a
 * cell's borders are painted by the *table*, so they do not travel with a sticky cell
 * and can be left behind at its unscrolled position. A shadow is painted by the cell
 * itself and is unaffected by the collapsed-border model.
 */
const pinnedEdge: Record<PinnedSide, string> = {
    left: 'sticky left-0 shadow-[1px_0_0_0_var(--border)]',
    right: 'sticky right-0 shadow-[-1px_0_0_0_var(--border)]',
};

/** Row hover tint. Solid, so a pinned cell can reproduce it exactly. */
export const ROW_HOVER = 'hover:bg-surface-2';

export const stickyHeaderCell = (side: PinnedSide): string =>
    cn(pinnedEdge[side], 'z-2 bg-surface-2');

export const stickyBodyCell = (side: PinnedSide): string =>
    cn(pinnedEdge[side], 'z-1 bg-surface group-hover:bg-surface-2');
