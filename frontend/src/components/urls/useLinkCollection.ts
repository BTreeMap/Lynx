import { useCallback, useEffect, useReducer, useRef } from 'react';
import { apiClient } from '../../api';
import { extractErrorMessage } from '../../utils/errorHandling';
import {
    ALL_LINKS,
    collectionReducer,
    initialCollection,
    listPage,
    searchPage,
    type CollectionState,
    type LinkPage,
    type LinkQuery,
} from './linkCollection';

/** Rows per request. The virtualiser bounds what is rendered, not what is fetched. */
export const PAGE_SIZE = 50;

/**
 * Pause between pages while draining. "Load all" is the one control that can issue an
 * unbounded number of requests, so it paces itself rather than emitting them as fast as
 * the server answers.
 */
const DRAIN_PACE_MS = 200;

/** An abortable pause: a new query must not have to wait out the previous one's pacing. */
const pause = (ms: number, signal: AbortSignal): Promise<void> =>
    new Promise((resolve) => {
        const timer = setTimeout(resolve, ms);
        signal.addEventListener(
            'abort',
            () => {
                clearTimeout(timer);
                resolve();
            },
            { once: true },
        );
    });

export const fetchLinkPage = (
    query: LinkQuery,
    cursor: string | null,
    signal: AbortSignal,
): Promise<LinkPage> =>
    query.tag === 'search'
        ? apiClient
              .searchUrls(
                  { ...query.filters, limit: PAGE_SIZE, cursor: cursor ?? undefined },
                  { signal },
              )
              .then(searchPage)
        : apiClient
              .listUrls({ limit: PAGE_SIZE, cursor: cursor ?? undefined, signal })
              .then(listPage);

export interface LinkCollection {
    readonly state: CollectionState;
    /** Start a new query, discarding the current result set. */
    readonly load: (query: LinkQuery) => void;
    /** Re-run the current query in place — after a mutation changed a row. */
    readonly refresh: () => void;
    readonly loadMore: () => void;
    /** Page until the collection is exhausted. */
    readonly loadAll: () => void;
}

/**
 * Interprets the collection machine against the API.
 *
 * One `AbortController` per query: starting a new one cancels the in-flight page *and*
 * any pacing pause, so a search issued during a drain stops the drain instead of racing
 * it. The request id carried through every event is the second half of that guarantee —
 * it rejects a settlement that was already in the network when the abort landed.
 */
export const useLinkCollection = (): LinkCollection => {
    const [state, dispatch] = useReducer(collectionReducer, initialCollection);
    const requestIdRef = useRef(initialCollection.requestId);
    const controllerRef = useRef<AbortController | null>(null);

    // The last controller is aborted on unmount; nothing may settle into a dead tree.
    useEffect(() => () => controllerRef.current?.abort(), []);

    /**
     * Start serving a query: cancel whatever the previous one had in flight, claim a new
     * request id, announce the transition, and fetch the first page under it.
     *
     * `announce` is the only difference between a fresh query and a refresh — the first
     * discards the rows on screen, the second keeps them — so the effect itself is
     * written once.
     */
    const start = useCallback(
        (query: LinkQuery, announce: (requestId: number) => void) => {
            controllerRef.current?.abort();
            const controller = new AbortController();
            controllerRef.current = controller;
            const requestId = requestIdRef.current + 1;
            requestIdRef.current = requestId;

            announce(requestId);
            fetchLinkPage(query, null, controller.signal).then(
                (page) => {
                    if (!controller.signal.aborted) {
                        dispatch({ type: 'firstPageLoaded', requestId, page });
                    }
                },
                (error: unknown) => {
                    if (controller.signal.aborted) return;
                    dispatch({
                        type: 'failed',
                        requestId,
                        message: extractErrorMessage(
                            error,
                            query.tag === 'search' ? 'Search failed' : 'Failed to load URLs',
                        ),
                    });
                },
            );
        },
        [],
    );

    const load = useCallback(
        (query: LinkQuery) =>
            start(query, (requestId) => dispatch({ type: 'queried', requestId, query })),
        [start],
    );

    const { phase, cursor, query, requestId } = state;

    const loadMore = useCallback(() => {
        const controller = controllerRef.current;
        if (phase.tag !== 'ready' || cursor === null || controller === null) return;

        dispatch({ type: 'appendRequested' });
        fetchLinkPage(query, cursor, controller.signal).then(
            (page) => {
                if (!controller.signal.aborted) {
                    dispatch({ type: 'pageAppended', requestId, page });
                }
            },
            (error: unknown) => {
                if (controller.signal.aborted) return;
                dispatch({
                    type: 'failed',
                    requestId,
                    message: extractErrorMessage(error, 'Failed to load more URLs'),
                });
            },
        );
    }, [phase, cursor, query, requestId]);

    const loadAll = useCallback(() => {
        const controller = controllerRef.current;
        if (phase.tag !== 'ready' || cursor === null || controller === null) return;
        const { signal } = controller;

        dispatch({ type: 'drainRequested' });

        // Sequential by construction: each page's cursor is the previous page's answer,
        // so this is a dependent chain and cannot be widened into a parallel fan-out.
        void (async () => {
            let next: string | null = cursor;
            try {
                while (next !== null && !signal.aborted) {
                    const page: LinkPage = await fetchLinkPage(query, next, signal);
                    if (signal.aborted) return;
                    dispatch({ type: 'pageAppended', requestId, page });
                    next = page.nextCursor;
                    if (next !== null) await pause(DRAIN_PACE_MS, signal);
                }
                if (!signal.aborted) dispatch({ type: 'drainFinished', requestId });
            } catch (error: unknown) {
                if (signal.aborted) return;
                dispatch({
                    type: 'failed',
                    requestId,
                    message: extractErrorMessage(error, 'Failed to load all URLs'),
                });
            }
        })();
    }, [phase, cursor, query, requestId]);

    const refresh = useCallback(
        () => start(query, (requestId) => dispatch({ type: 'refreshed', requestId })),
        [start, query],
    );

    // The initial query. Depends on `load`, which is stable, so it runs exactly once.
    useEffect(() => {
        load(ALL_LINKS);
    }, [load]);

    return { state, load, refresh, loadMore, loadAll };
};
