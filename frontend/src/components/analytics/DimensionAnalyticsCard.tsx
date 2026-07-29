import React, { useMemo, useState } from 'react';
import { foldRemote } from '../../lib/remoteData';
import { Card, CardBody, CardSectionHeader, CardTitle } from '../ui/Card';
import { EmptyState } from '../ui/EmptyState';
import { SegmentedControl } from '../ui/SegmentedControl';
import { Skeleton } from '../ui/Skeleton';
import { Table, TBody, TD, TH, THead, TR, TableScroll } from '../ui/Table';
import { DimensionCharts } from './DimensionCharts';
import {
    CHART_SLICE_LIMIT,
    DIMENSION_LABELS,
    DIMENSIONS,
    toDimensionSlices,
    type AggregateDimension,
    type DimensionSlice,
} from './dimensions';
import { useDimensionAggregate } from './useAnalytics';

const DistributionBar: React.FC<{ readonly slice: DimensionSlice }> = ({ slice }) => (
    <div className="flex items-center gap-3">
        <div className="h-2 flex-1 overflow-hidden rounded-full bg-surface-2">
            <div
                className={
                    slice.isOther
                        ? 'h-full rounded-full bg-fg-subtle'
                        : 'h-full rounded-full bg-primary'
                }
                style={{ width: `${slice.share}%` }}
            />
        </div>
        <span className="w-12 text-right text-xs text-fg-subtle tabular-nums sm:text-sm">
            {slice.share.toFixed(1)}%
        </span>
    </div>
);

export interface DimensionAnalyticsCardProps {
    /** Decoded short code, or `null` when the route parameter did not parse. */
    readonly code: string | null;
}

/**
 * Visits grouped by a dimension: the selector, the two charts, and the full table.
 *
 * The selected dimension is state *of this card*, held here rather than in the route,
 * because nothing outside it reads the choice — and the query that depends on it is
 * keyed by it, so selecting a new dimension cancels the request for the previous one.
 */
export const DimensionAnalyticsCard: React.FC<DimensionAnalyticsCardProps> = ({ code }) => {
    const [dimension, setDimension] = useState<AggregateDimension>('country');
    const aggregate = useDimensionAggregate(code, dimension);

    const label = DIMENSION_LABELS[dimension];

    const slices = useMemo(
        () =>
            aggregate.tag === 'success'
                ? toDimensionSlices(aggregate.value.aggregates, aggregate.value.clicks, dimension)
                : [],
        [aggregate, dimension],
    );

    const empty = (
        <EmptyState
            title={`No ${label.toLowerCase()} data yet`}
            description="Analytics will appear here once your link starts receiving visits."
        />
    );

    return (
        <Card>
            <CardSectionHeader
                actions={
                    <SegmentedControl
                        ariaLabel="Group analytics by"
                        options={[...DIMENSIONS]}
                        value={dimension}
                        onChange={setDimension}
                    />
                }
            >
                <CardTitle>Analytics by dimension</CardTitle>
            </CardSectionHeader>
            <CardBody>
                {foldRemote(aggregate, {
                    // `idle` is unreachable from this card's own rendering — the route
                    // shows an error page when the code fails to parse — but the
                    // eliminator is total, so it is spelled out rather than assumed.
                    onIdle: () => empty,
                    onLoading: () => <Skeleton className="h-72 w-full" />,
                    // Recovered upstream into an empty response, so this cannot be
                    // reached; the branch exists to keep the match exhaustive.
                    onFailure: () => empty,
                    onSuccess: () =>
                        slices.length === 0 ? (
                            empty
                        ) : (
                            <div className="space-y-5 sm:space-y-6">
                                <DimensionCharts
                                    slices={slices.slice(0, CHART_SLICE_LIMIT)}
                                    barTitle={`Top ${label.toLowerCase()} by visits`}
                                />

                                <TableScroll className="shadow-none">
                                    <Table>
                                        <THead>
                                            <TR className="border-b-0">
                                                <TH>{label}</TH>
                                                <TH className="text-right">Visits</TH>
                                                <TH className="w-1/2">Distribution</TH>
                                            </TR>
                                        </THead>
                                        <TBody>
                                            {slices.map((slice) => (
                                                <TR key={slice.key}>
                                                    <TD
                                                        className={
                                                            slice.isOther
                                                                ? 'italic text-fg-muted'
                                                                : 'font-medium text-fg'
                                                        }
                                                    >
                                                        {slice.label}
                                                    </TD>
                                                    <TD className="text-right font-medium tabular-nums">
                                                        {slice.visits.toLocaleString()}
                                                    </TD>
                                                    <TD>
                                                        <DistributionBar slice={slice} />
                                                    </TD>
                                                </TR>
                                            ))}
                                        </TBody>
                                    </Table>
                                </TableScroll>
                            </div>
                        ),
                })}
            </CardBody>
        </Card>
    );
};
