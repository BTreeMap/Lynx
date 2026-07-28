/** Rendered in place of an absent principal (unauthenticated or legacy rows). */
export const ANONYMOUS_PRINCIPAL = '—';

const ELLIPSIS = '…';

/**
 * Shorten an opaque principal (UUID, OIDC subject) so it fits a bounded column.
 *
 * Keeps a head *and* a tail: principals issued by the same provider often share a
 * prefix, so a head-only truncation would render distinct users identically. The
 * result is lossy by construction — every caller must also expose the full value
 * (tooltip, copy affordance) rather than treating this as the identifier.
 */
export const shortenPrincipal = (value: string, head = 8, tail = 4): string =>
    value.length <= head + tail + ELLIPSIS.length
        ? value
        : `${value.slice(0, head)}${ELLIPSIS}${value.slice(-tail)}`;
