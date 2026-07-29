import type { AnalyticsAggregate } from '../../types';

/**
 * The dimension a link's visits can be grouped by, and how a grouped result is turned
 * into rows fit to render.
 *
 * Framework-free and pure: the card, the charts and the table all read the same derived
 * slices, so they cannot disagree about a label, a share, or which row is the
 * remainder.
 */

export type AggregateDimension = 'country' | 'region' | 'city' | 'asn' | 'hour' | 'day';

export interface DimensionOption {
    readonly value: AggregateDimension;
    readonly label: string;
}

export const DIMENSIONS: readonly DimensionOption[] = [
    { value: 'country', label: 'Country' },
    { value: 'region', label: 'Region' },
    { value: 'city', label: 'City' },
    { value: 'asn', label: 'ASN' },
    { value: 'hour', label: 'Hour' },
    { value: 'day', label: 'Day' },
];

/**
 * A `Record` over the closed union rather than a lookup that can miss: adding a
 * dimension without labelling it is a compile error, not a blank heading.
 */
export const DIMENSION_LABELS: Readonly<Record<AggregateDimension, string>> = {
    country: 'Country',
    region: 'Region',
    city: 'City',
    asn: 'ASN',
    hour: 'Hour',
    day: 'Day',
};

/** Bucket timestamps arrive as seconds; the time dimensions render them as dates. */
const formatBucket = (value: string, dimension: 'hour' | 'day'): string | null => {
    const timestamp = Number.parseInt(value, 10);
    if (Number.isNaN(timestamp)) {
        return null;
    }
    const date = new Date(timestamp * 1000);
    return dimension === 'hour'
        ? date.toLocaleString([], {
              month: 'short',
              day: 'numeric',
              hour: '2-digit',
              minute: '2-digit',
          })
        : date.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' });
};

export const formatDimensionValue = (value: string, dimension: AggregateDimension): string => {
    if (dimension === 'hour' || dimension === 'day') {
        return formatBucket(value, dimension) ?? (value || 'Unknown');
    }
    return value || 'Unknown';
};

/**
 * One aggregate row, ready to render.
 *
 * `isOther` is a field rather than a comparison against the literal `'Other'`. The
 * remainder row used to be recognised by its label, so a genuine dimension value of
 * "Other" — a real country name in some datasets, and the literal ASN fallback — was
 * styled and coloured as the remainder. A tag decided where the row was constructed
 * cannot be forged by the data.
 */
export interface DimensionSlice {
    readonly key: string;
    readonly label: string;
    readonly visits: number;
    readonly isOther: boolean;
    /** Percentage of all clicks, `0` when the total is unknown. */
    readonly share: number;
}

const OTHER_LABEL = 'Other';

/**
 * Turn a grouped response into display rows, appending the unaccounted remainder.
 *
 * The server returns the top *n* groups while `clicks` counts every visit, so the two
 * disagree by however much the tail holds. Making that difference an explicit row is
 * what stops the chart from implying the top groups are the whole story.
 *
 * A fold with identity `0` over `visit_count` supplies the accounted total; the
 * remainder is admitted only when positive, since a negative difference would mean the
 * totals were read from two different responses.
 */
export const toDimensionSlices = (
    aggregates: readonly AnalyticsAggregate[],
    totalClicks: number,
    dimension: AggregateDimension,
): readonly DimensionSlice[] => {
    const share = (visits: number) => (totalClicks > 0 ? (visits / totalClicks) * 100 : 0);

    const slices = aggregates.map((aggregate, index) => ({
        key: `${index}:${aggregate.dimension}`,
        label: formatDimensionValue(aggregate.dimension, dimension),
        visits: aggregate.visit_count,
        isOther: false,
        share: share(aggregate.visit_count),
    }));

    if (aggregates.length === 0 || totalClicks === 0) {
        return slices;
    }

    const accounted = aggregates.reduce((sum, entry) => sum + entry.visit_count, 0);
    const remainder = totalClicks - accounted;
    return remainder > 0
        ? [
              ...slices,
              {
                  key: 'other',
                  label: OTHER_LABEL,
                  visits: remainder,
                  isOther: true,
                  share: share(remainder),
              },
          ]
        : slices;
};

/** How many slices the charts draw; the table lists them all. */
export const CHART_SLICE_LIMIT = 10;
