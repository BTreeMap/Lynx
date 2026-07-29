import { describe, expect, it } from 'vitest';
import { DIMENSION_LABELS, DIMENSIONS, formatDimensionValue, toDimensionSlices } from './dimensions';

describe('dimension options', () => {
    it('labels every offered dimension', () => {
        for (const option of DIMENSIONS) {
            expect(DIMENSION_LABELS[option.value]).toBe(option.label);
        }
    });
});

describe('formatDimensionValue', () => {
    it('renders an absent value as Unknown', () => {
        expect(formatDimensionValue('', 'country')).toBe('Unknown');
        expect(formatDimensionValue('', 'asn')).toBe('Unknown');
    });

    it('passes a categorical value through', () => {
        expect(formatDimensionValue('CA', 'country')).toBe('CA');
    });

    it('falls back to the raw value when a time bucket will not parse', () => {
        expect(formatDimensionValue('not-a-timestamp', 'hour')).toBe('not-a-timestamp');
        expect(formatDimensionValue('', 'day')).toBe('Unknown');
    });

    it('formats a time bucket as a date', () => {
        // Locale-dependent, so this asserts only that a bucket is interpreted as seconds
        // and reformatted rather than shown as a raw epoch.
        const formatted = formatDimensionValue('1700000000', 'day');
        expect(formatted).not.toBe('1700000000');
        expect(formatted).toMatch(/2023/);
    });
});

describe('toDimensionSlices', () => {
    it('appends the unaccounted remainder', () => {
        const slices = toDimensionSlices([{ dimension: 'CA', visit_count: 30 }], 100, 'country');
        expect(slices).toEqual([
            { key: '0:CA', label: 'CA', visits: 30, isOther: false, share: 30 },
            { key: 'other', label: 'Other', visits: 70, isOther: true, share: 70 },
        ]);
    });

    it('omits the remainder when the groups account for every click', () => {
        expect(toDimensionSlices([{ dimension: 'CA', visit_count: 100 }], 100, 'country')).toHaveLength(1);
    });

    it('omits the remainder when the totals disagree in the other direction', () => {
        // A negative difference would mean the groups and the total came from different
        // responses; inventing a negative slice would corrupt the chart.
        expect(toDimensionSlices([{ dimension: 'CA', visit_count: 120 }], 100, 'country')).toHaveLength(1);
    });

    it('reports no shares when the total is unknown', () => {
        expect(toDimensionSlices([{ dimension: 'CA', visit_count: 3 }], 0, 'country')).toEqual([
            { key: '0:CA', label: 'CA', visits: 3, isOther: false, share: 0 },
        ]);
    });

    it('does not mistake a dimension named "Other" for the remainder', () => {
        // The remainder used to be recognised by its label, so a real value of that name —
        // a country in some datasets, and the ASN fallback — was styled as the remainder.
        const slices = toDimensionSlices([{ dimension: 'Other', visit_count: 100 }], 100, 'country');
        expect(slices).toHaveLength(1);
        expect(slices[0].isOther).toBe(false);
    });

    it('gives colliding labels distinct keys', () => {
        const slices = toDimensionSlices(
            [
                { dimension: '', visit_count: 1 },
                { dimension: '', visit_count: 1 },
            ],
            0,
            'city',
        );
        expect(slices.map((slice) => slice.label)).toEqual(['Unknown', 'Unknown']);
        expect(new Set(slices.map((slice) => slice.key)).size).toBe(2);
    });

    it('returns nothing for an empty result', () => {
        expect(toDimensionSlices([], 50, 'country')).toEqual([]);
    });
});
