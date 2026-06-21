/** Format a Unix timestamp (seconds) as a locale-aware date-time string. */
export const formatDate = (timestamp: number): string =>
    new Date(timestamp * 1000).toLocaleString();
