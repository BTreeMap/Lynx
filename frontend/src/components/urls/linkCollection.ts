import { assertNever } from '../../lib/assertNever';
import type { PaginatedUrlsResponse, SearchResponse, ShortenedUrl } from '../../types';

/**
 * The dashboard's link collection, as a state machine.
 *
 * It replaces nine independent `useState` cells (`urls`, `isLoading`, `isLoadingMore`,
 * `isLoadingAll`, `isSearching`, `error`, `nextCursor`, `hasMore`, `activeFilters`)
 * whose combinations included states no sequence of user actions should reach —
 * "searching while draining every page", "more pages available with no cursor to fetch
 * them with" — and, more damagingly, offered no way to tell a response apart from a
 * stale one: a list request still in flight when a search was submitted would overwrite
 * the search results on arrival.
 *
 * Everything here is pure. The interpreter lives in `./useLinkCollection.ts`.
 */

/* ---------------------------------------------------------------------------
   What is being listed
--------------------------------------------------------------------------- */

/** Filters as the search panel produces them, sans pagination. */
export interface SearchFilters {
    readonly q: string;
    readonly created_by?: string;
    readonly created_from?: number;
    readonly created_to?: number;
    readonly is_active?: boolean;
}

/**
 * Two sources, one shape. The distinction used to be carried by a nullable
 * `activeFilters`, which every paginating branch had to re-test — and each such test
 * was a chance for the "load more" call to page a different query than the one on
 * screen.
 */
export type LinkQuery =
    | { readonly tag: 'all' }
    | { readonly tag: 'search'; readonly filters: SearchFilters };

export const ALL_LINKS: LinkQuery = { tag: 'all' };

/**
 * A page, with pagination collapsed to a single `Option`.
 *
 * The wire carries `has_more` *and* `next_cursor`, which together admit "more pages
 * exist but there is no cursor for them" — a state that rendered a Load more control
 * whose handler returned immediately. One nullable cursor cannot express it: a cursor
 * is exactly the permission to ask for another page.
 */
export interface LinkPage {
    readonly items: readonly ShortenedUrl[];
    readonly nextCursor: string | null;
}

const pageCursor = (response: {
    readonly next_cursor?: string | null;
    readonly has_more: boolean;
}): string | null => (response.has_more ? (response.next_cursor ?? null) : null);

export const listPage = (response: PaginatedUrlsResponse): LinkPage => ({
    items: response.urls,
    nextCursor: pageCursor(response),
});

export const searchPage = (response: SearchResponse): LinkPage => ({
    items: response.items,
    nextCursor: pageCursor(response),
});

/* ---------------------------------------------------------------------------
   State
--------------------------------------------------------------------------- */

/**
 * Exactly one load can be in flight, and this says which. Whether the first page is a
 * list or a search is *not* a phase: it is a property of `query`, and duplicating it
 * here would let the two disagree.
 */
export type LoadPhase =
    | { readonly tag: 'first' }
    | { readonly tag: 'ready' }
    | { readonly tag: 'appending' }
    | { readonly tag: 'draining' };

export interface CollectionState {
    /**
     * Identity of the query being served. Every settlement carries the id it was issued
     * under, so a response that outlived its query is discarded by the reducer instead
     * of racing the one that replaced it.
     */
    readonly requestId: number;
    readonly query: LinkQuery;
    readonly items: readonly ShortenedUrl[];
    /** Cursor for the next page; `null` means the collection is complete. */
    readonly cursor: string | null;
    readonly phase: LoadPhase;
    /** Survives across loads on purpose: a failed page should stay reported. */
    readonly error: string | null;
}

export const initialCollection: CollectionState = {
    requestId: 0,
    query: ALL_LINKS,
    items: [],
    cursor: null,
    phase: { tag: 'first' },
    error: null,
};

export type CollectionEvent =
    | { readonly type: 'queried'; readonly requestId: number; readonly query: LinkQuery }
    /** Re-read the *same* query — the rows on screen stay until the answer arrives. */
    | { readonly type: 'refreshed'; readonly requestId: number }
    | { readonly type: 'appendRequested' }
    | { readonly type: 'drainRequested' }
    | { readonly type: 'firstPageLoaded'; readonly requestId: number; readonly page: LinkPage }
    | { readonly type: 'pageAppended'; readonly requestId: number; readonly page: LinkPage }
    | { readonly type: 'drainFinished'; readonly requestId: number }
    | { readonly type: 'failed'; readonly requestId: number; readonly message: string };

/** True when the event belongs to the query currently on screen. */
const isCurrent = (state: CollectionState, requestId: number): boolean =>
    state.requestId === requestId;

export const collectionReducer = (
    state: CollectionState,
    event: CollectionEvent,
): CollectionState => {
    switch (event.type) {
        case 'queried':
            // A new query is a new collection: dropping the items here is what stops the
            // previous result set from being appended to by a page already in flight.
            return {
                requestId: event.requestId,
                query: event.query,
                items: [],
                cursor: null,
                phase: { tag: 'first' },
                error: null,
            };

        case 'refreshed':
            // Unlike `queried`, the items are kept: a refresh answers the same question,
            // so discarding the current answer would blank the list — and, under an
            // active search, briefly claim there were no matches.
            return { ...state, requestId: event.requestId, phase: { tag: 'first' }, error: null };

        case 'appendRequested':
            return state.phase.tag === 'ready' && state.cursor !== null
                ? { ...state, phase: { tag: 'appending' }, error: null }
                : state;

        case 'drainRequested':
            return state.phase.tag === 'ready' && state.cursor !== null
                ? { ...state, phase: { tag: 'draining' }, error: null }
                : state;

        case 'firstPageLoaded':
            return isCurrent(state, event.requestId) && state.phase.tag === 'first'
                ? {
                      ...state,
                      items: event.page.items,
                      cursor: event.page.nextCursor,
                      phase: { tag: 'ready' },
                  }
                : state;

        case 'pageAppended': {
            if (
                !isCurrent(state, event.requestId) ||
                (state.phase.tag !== 'appending' && state.phase.tag !== 'draining')
            ) {
                return state;
            }
            const exhausted = event.page.nextCursor === null;
            return {
                ...state,
                items: [...state.items, ...event.page.items],
                cursor: event.page.nextCursor,
                // A drain keeps going until the cursor runs out; a single append is done.
                phase:
                    state.phase.tag === 'draining' && !exhausted
                        ? { tag: 'draining' }
                        : { tag: 'ready' },
            };
        }

        case 'drainFinished':
            return isCurrent(state, event.requestId) && state.phase.tag === 'draining'
                ? { ...state, phase: { tag: 'ready' } }
                : state;

        case 'failed':
            return isCurrent(state, event.requestId)
                ? { ...state, phase: { tag: 'ready' }, error: event.message }
                : state;

        default:
            return assertNever(event);
    }
};

/* ---------------------------------------------------------------------------
   Selectors
--------------------------------------------------------------------------- */

export const hasMorePages = (state: CollectionState): boolean => state.cursor !== null;

/** True while the first page of an *unfiltered* list is loading — the skeleton case. */
export const isLoadingList = (state: CollectionState): boolean =>
    state.phase.tag === 'first' && state.query.tag === 'all';

/** True while the first page of a *search* is loading — the busy-button case. */
export const isSearching = (state: CollectionState): boolean =>
    state.phase.tag === 'first' && state.query.tag === 'search';

export const isPaging = (state: CollectionState): boolean =>
    state.phase.tag === 'appending' || state.phase.tag === 'draining';

export const isDraining = (state: CollectionState): boolean => state.phase.tag === 'draining';

/** Filters currently applied, or `null` when the full list is shown. */
export const activeFilters = (state: CollectionState): SearchFilters | null =>
    state.query.tag === 'search' ? state.query.filters : null;

export interface CollectionStats {
    readonly count: number;
    readonly totalClicks: number;
    readonly active: number;
    readonly inactive: number;
}

/**
 * One traversal instead of three, over the loaded rows only — which is what the cards
 * say ("Clicks (shown)").
 *
 * A fold with a pair accumulator, compiled to the loop that is its honest backend: the
 * combining function is `(clicks, active) ↦ (clicks + n, active + [is_active])`, with
 * identity `(0, 0)`, and expressing it as `reduce` over a record would allocate one
 * intermediate object per link — a cost that lands precisely when "Load all" has pulled
 * in thousands of them. The mutation never escapes this function, so the result is
 * indistinguishable from the pure fold.
 */
export const summarise = (items: readonly ShortenedUrl[]): CollectionStats => {
    let totalClicks = 0;
    let active = 0;
    for (const url of items) {
        totalClicks += url.clicks;
        if (url.is_active) active += 1;
    }
    return { count: items.length, totalClicks, active, inactive: items.length - active };
};
