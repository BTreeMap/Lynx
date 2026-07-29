import { describe, expect, it } from 'vitest';
import type { ShortenedUrl } from '../../types';
import {
    activeFilters,
    ALL_LINKS,
    collectionReducer,
    hasMorePages,
    initialCollection,
    isDraining,
    isLoadingList,
    isPaging,
    isSearching,
    listPage,
    searchPage,
    summarise,
    type CollectionState,
    type LinkPage,
    type LinkQuery,
} from './linkCollection';

const link = (id: number, overrides: Partial<ShortenedUrl> = {}): ShortenedUrl => ({
    id,
    short_code: `c${id}`,
    original_url: `https://example.com/${id}`,
    created_at: 1_700_000_000 + id,
    created_by: null,
    clicks: id,
    is_active: id % 2 === 0,
    ...overrides,
});

const pageOf = (ids: readonly number[], nextCursor: string | null): LinkPage => ({
    items: ids.map((id) => link(id)),
    nextCursor,
});

const SEARCH: LinkQuery = { tag: 'search', filters: { q: 'promo' } };

/** Drive the machine to a settled first page. */
const loaded = (query: LinkQuery, page: LinkPage, requestId = 1): CollectionState =>
    collectionReducer(
        collectionReducer(initialCollection, { type: 'queried', requestId, query }),
        { type: 'firstPageLoaded', requestId, page },
    );

describe('page normalisation', () => {
    it('treats "more pages" without a cursor as the end', () => {
        // The pair (has_more, next_cursor) admits a state that rendered a Load more
        // control whose handler could only return immediately.
        expect(listPage({ urls: [], has_more: true, next_cursor: null }).nextCursor).toBeNull();
        expect(searchPage({ items: [], has_more: true }).nextCursor).toBeNull();
    });

    it('ignores a cursor the server did not authorise', () => {
        expect(listPage({ urls: [], has_more: false, next_cursor: 'c' }).nextCursor).toBeNull();
    });

    it('keeps a cursor that comes with permission to use it', () => {
        expect(listPage({ urls: [], has_more: true, next_cursor: 'c' }).nextCursor).toBe('c');
        expect(searchPage({ items: [], has_more: true, next_cursor: 'c' }).nextCursor).toBe('c');
    });
});

describe('starting a query', () => {
    it('distinguishes a loading list from a loading search', () => {
        const listing = collectionReducer(initialCollection, {
            type: 'queried',
            requestId: 1,
            query: ALL_LINKS,
        });
        expect(isLoadingList(listing)).toBe(true);
        expect(isSearching(listing)).toBe(false);
        expect(activeFilters(listing)).toBeNull();

        const searching = collectionReducer(initialCollection, {
            type: 'queried',
            requestId: 2,
            query: SEARCH,
        });
        expect(isSearching(searching)).toBe(true);
        expect(isLoadingList(searching)).toBe(false);
        expect(activeFilters(searching)).toEqual({ q: 'promo' });
    });

    it('discards the previous answer', () => {
        const state = loaded(ALL_LINKS, pageOf([1, 2], 'k'));
        const searching = collectionReducer(state, { type: 'queried', requestId: 2, query: SEARCH });
        expect(searching.items).toHaveLength(0);
        expect(searching.cursor).toBeNull();
    });

    it('keeps the rows on screen across a refresh of the same query', () => {
        // Blanking here would, under an active search, briefly claim there were no
        // matches for a query that has some.
        const state = loaded(SEARCH, pageOf([1, 2, 3], null));
        const refreshed = collectionReducer(state, { type: 'refreshed', requestId: 2 });
        expect(refreshed.items).toHaveLength(3);
        expect(refreshed.phase).toEqual({ tag: 'first' });

        const settled = collectionReducer(refreshed, {
            type: 'firstPageLoaded',
            requestId: 2,
            page: pageOf([9], null),
        });
        expect(settled.items.map((item) => item.id)).toEqual([9]);
    });
});

describe('stale settlements', () => {
    it('discards a page belonging to a superseded query', () => {
        const state = collectionReducer(loaded(ALL_LINKS, pageOf([1, 2], 'k')), {
            type: 'queried',
            requestId: 2,
            query: SEARCH,
        });
        const stale = collectionReducer(state, {
            type: 'firstPageLoaded',
            requestId: 1,
            page: pageOf([7, 8], 'k'),
        });
        expect(stale).toBe(state);
    });

    it('discards a stale append and a stale failure', () => {
        const state = loaded(ALL_LINKS, pageOf([1], 'k'));
        expect(collectionReducer(state, { type: 'pageAppended', requestId: 0, page: pageOf([2], null) })).toBe(state);
        expect(collectionReducer(state, { type: 'failed', requestId: 0, message: 'old' })).toBe(state);
    });
});

describe('paging', () => {
    it('appends one page and settles', () => {
        let state = loaded(ALL_LINKS, pageOf([1, 2], 'a'));
        expect(hasMorePages(state)).toBe(true);

        state = collectionReducer(state, { type: 'appendRequested' });
        expect(isPaging(state)).toBe(true);
        state = collectionReducer(state, { type: 'pageAppended', requestId: 1, page: pageOf([3], 'b') });
        expect(state.items.map((item) => item.id)).toEqual([1, 2, 3]);
        expect(state.phase).toEqual({ tag: 'ready' });
        expect(state.cursor).toBe('b');
    });

    it('refuses a second request while one is in flight', () => {
        const appending = collectionReducer(loaded(ALL_LINKS, pageOf([1], 'a')), {
            type: 'appendRequested',
        });
        expect(collectionReducer(appending, { type: 'appendRequested' })).toBe(appending);
        expect(collectionReducer(appending, { type: 'drainRequested' })).toBe(appending);
    });

    it('refuses to page when there is nothing left', () => {
        const complete = loaded(ALL_LINKS, pageOf([1], null));
        expect(hasMorePages(complete)).toBe(false);
        expect(collectionReducer(complete, { type: 'appendRequested' })).toBe(complete);
        expect(collectionReducer(complete, { type: 'drainRequested' })).toBe(complete);
    });

    it('drains until the cursor runs out', () => {
        let state = collectionReducer(loaded(ALL_LINKS, pageOf([1], 'a')), {
            type: 'drainRequested',
        });
        expect(isDraining(state)).toBe(true);

        state = collectionReducer(state, { type: 'pageAppended', requestId: 1, page: pageOf([2], 'b') });
        expect(isDraining(state)).toBe(true);

        state = collectionReducer(state, { type: 'pageAppended', requestId: 1, page: pageOf([3], null) });
        expect(isDraining(state)).toBe(false);
        expect(state.phase).toEqual({ tag: 'ready' });
        expect(state.items.map((item) => item.id)).toEqual([1, 2, 3]);
    });

    it('closes out a drain that stopped early', () => {
        const draining = collectionReducer(loaded(ALL_LINKS, pageOf([1], 'a')), {
            type: 'drainRequested',
        });
        const finished = collectionReducer(draining, { type: 'drainFinished', requestId: 1 });
        expect(finished.phase).toEqual({ tag: 'ready' });
        // ...but only from `draining`.
        expect(collectionReducer(finished, { type: 'drainFinished', requestId: 1 })).toBe(finished);
    });
});

describe('failure', () => {
    it('settles the phase and reports the message', () => {
        const state = collectionReducer(
            collectionReducer(initialCollection, { type: 'queried', requestId: 1, query: ALL_LINKS }),
            { type: 'failed', requestId: 1, message: 'Failed to load URLs' },
        );
        expect(state.phase).toEqual({ tag: 'ready' });
        expect(state.error).toBe('Failed to load URLs');
    });

    it('clears when the next page is requested', () => {
        const failed = collectionReducer(loaded(ALL_LINKS, pageOf([1], 'a')), {
            type: 'failed',
            requestId: 1,
            message: 'boom',
        });
        expect(collectionReducer(failed, { type: 'appendRequested' }).error).toBeNull();
    });
});

describe('summarise', () => {
    it('is the identity fold over an empty collection', () => {
        expect(summarise([])).toEqual({ count: 0, totalClicks: 0, active: 0, inactive: 0 });
    });

    it('counts clicks and statuses in one pass', () => {
        expect(summarise([1, 2, 3, 4, 5].map((id) => link(id)))).toEqual({
            count: 5,
            totalClicks: 1 + 2 + 3 + 4 + 5,
            active: 2, // ids 2 and 4
            inactive: 3,
        });
    });
});
