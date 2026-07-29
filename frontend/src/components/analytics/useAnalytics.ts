import { useEffect, useMemo } from 'react';
import { apiClient } from '../../api';
import { useRemoteQuery, type Fetcher } from '../../hooks/useRemoteQuery';
import { recover, success, type RemoteData } from '../../lib/remoteData';
import type { AnalyticsAggregateResponse, AnalyticsResponse } from '../../types';
import type { AggregateDimension } from './dimensions';

/**
 * Analytics reads.
 *
 * Both are *degradable*: an instance may not be collecting analytics at all, and the
 * endpoints then fail rather than return nothing. The screen has always treated that
 * exactly like "no visits yet", so the failure is recovered into an empty response here
 * — at the boundary, once — instead of leaving every consumer to decide again. The
 * failure is still logged, because "not collecting" and "briefly unreachable" look the
 * same from the component and only one of them is worth investigating.
 */

/** Groups requested per dimension. The table lists them all; the charts draw the top 10. */
const AGGREGATE_LIMIT = 20;

/** Rows of recent activity. Every row fetched is rendered. */
const RECENT_LIMIT = 20;

/* Hoisted so a recovered failure keeps a stable identity across renders. */
const EMPTY_AGGREGATE: RemoteData<AnalyticsAggregateResponse> = success({
    aggregates: [],
    total: 0,
    clicks: 0,
});
const EMPTY_RECENT: RemoteData<AnalyticsResponse> = success({
    entries: [],
    total: 0,
    clicks: 0,
});

const useLoggedFailure = (state: RemoteData<unknown>, label: string): void => {
    useEffect(() => {
        if (state.tag === 'failure') {
            console.warn(`${label}:`, state.message);
        }
    }, [state, label]);
};

/**
 * Visits grouped by one dimension.
 *
 * The response carries its own `clicks` total, and the shares are computed against
 * *that* number. Two effects used to write one shared `totalClicks` cell — one from this
 * request, one from the recent-activity request — so the denominator of every percentage
 * depended on which of two unrelated responses landed last.
 */
export const useDimensionAggregate = (
    code: string | null,
    dimension: AggregateDimension,
): RemoteData<AnalyticsAggregateResponse> => {
    const fetcher = useMemo<Fetcher<AnalyticsAggregateResponse> | null>(
        () =>
            code === null
                ? null
                : (signal) =>
                      apiClient.getAnalyticsAggregate(code, {
                          groupBy: dimension,
                          limit: AGGREGATE_LIMIT,
                          signal,
                      }),
        [code, dimension],
    );

    const { state } = useRemoteQuery(fetcher, 'Analytics aggregates are unavailable');
    useLoggedFailure(state, 'Analytics aggregates not available');
    return recover(state, EMPTY_AGGREGATE);
};

export const useRecentActivity = (code: string | null): RemoteData<AnalyticsResponse> => {
    const fetcher = useMemo<Fetcher<AnalyticsResponse> | null>(
        () =>
            code === null
                ? null
                : (signal) => apiClient.getAnalytics(code, { limit: RECENT_LIMIT, signal }),
        [code],
    );

    const { state } = useRemoteQuery(fetcher, 'Analytics data is unavailable');
    useLoggedFailure(state, 'Analytics data not available');
    return recover(state, EMPTY_RECENT);
};
