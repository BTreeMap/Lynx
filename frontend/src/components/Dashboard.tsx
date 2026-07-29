import React, { useCallback, useEffect, useMemo, useRef } from 'react';
import { Download, Search as SearchIcon } from 'lucide-react';
import { isAdmin as selectIsAdmin } from '../auth/model';
import { useAuth } from '../hooks/useAuth';
import CreateUrlForm from './CreateUrlForm';
import SearchPanel from './SearchPanel';
import UrlList from './UrlList';
import { DashboardStats } from './dashboard/DashboardStats';
import { AppHeader } from './layout/AppHeader';
import { PageIntro, PageShell } from './layout/Page';
import { Alert } from './ui/Alert';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';
import { EmptyState } from './ui/EmptyState';
import { Skeleton } from './ui/Skeleton';
import { Spinner } from './ui/Spinner';
import {
    ALL_LINKS,
    activeFilters,
    hasMorePages,
    isDraining,
    isLoadingList,
    isPaging,
    isSearching,
    summarise,
    type SearchFilters,
} from './urls/linkCollection';
import { useLinkExport } from './urls/exportLinks';
import { PAGE_SIZE, useLinkCollection } from './urls/useLinkCollection';

/** Distance from the sentinel at which the next page starts loading. */
const PREFETCH_MARGIN = '320px 0px';

const Dashboard: React.FC = () => {
    const { state: auth } = useAuth();
    const isAdmin = selectIsAdmin(auth);

    const collection = useLinkCollection();
    const { state, load, refresh, loadMore, loadAll } = collection;
    const exporter = useLinkExport();

    const sentinelRef = useRef<HTMLDivElement | null>(null);

    const filters = activeFilters(state);
    const hasMore = hasMorePages(state);
    const stats = useMemo(() => summarise(state.items), [state.items]);

    const search = useCallback((next: SearchFilters) => load({ tag: 'search', filters: next }), [load]);
    const clearSearch = useCallback(() => load(ALL_LINKS), [load]);

    // Infinite scroll. The observer is re-attached whenever the pager changes identity,
    // which is what keeps it paging the query currently on screen.
    useEffect(() => {
        const node = sentinelRef.current;
        if (!node || !hasMore) return;
        const observer = new IntersectionObserver(
            (entries) => {
                if (entries[0]?.isIntersecting) loadMore();
            },
            { rootMargin: PREFETCH_MARGIN },
        );
        observer.observe(node);
        return () => observer.disconnect();
    }, [hasMore, loadMore]);

    /*
      Both error channels render in the same slot, as they did when they shared one
      `error` cell — but they are now owned by the machines that produce them, so a
      failed export cannot be cleared by a successful page load.
    */
    const error = state.error ?? exporter.error;

    return (
        <div className="min-h-screen bg-bg">
            <AppHeader
                actions={
                    <Button
                        variant="secondary"
                        size="sm"
                        onClick={exporter.exportJson}
                        isLoading={exporter.isExporting}
                        leftIcon={
                            !exporter.isExporting ? <Download className="h-4 w-4" /> : undefined
                        }
                    >
                        <span className="hidden sm:inline">Export JSON</span>
                        <span className="sm:hidden">Export</span>
                    </Button>
                }
            />

            <PageShell>
                <PageIntro
                    title="Dashboard"
                    description="Create, manage, and track your short links."
                />

                <DashboardStats
                    stats={stats}
                    filtered={filters !== null}
                    hasMore={hasMore}
                    isDraining={isDraining(state)}
                    canLoadAll={!isPaging(state)}
                    onLoadAll={loadAll}
                />

                {/* A new link is shown by returning to the unfiltered list: it need not
                    match the search that happens to be active. */}
                <CreateUrlForm onUrlCreated={clearSearch} />

                <section className="space-y-3 sm:space-y-4">
                    <div className="flex flex-wrap items-end justify-between gap-2.5 sm:gap-3">
                        <div>
                            <h2 className="text-lg font-semibold tracking-tight text-fg">
                                Your links
                            </h2>
                            <p className="mt-1 text-sm text-fg-muted">
                                {filters
                                    ? `Showing results for “${filters.q}”`
                                    : 'All links you have created.'}
                            </p>
                        </div>
                        {filters && (
                            <Badge tone="primary" className="gap-2 py-1 pl-2.5 pr-1.5">
                                {stats.count}
                                {hasMore ? '+' : ''} result{stats.count === 1 ? '' : 's'}
                                <button
                                    type="button"
                                    onClick={clearSearch}
                                    className="rounded-full px-2 py-0.5 text-xs font-medium text-primary-soft-fg/80 underline-offset-2 hover:underline"
                                >
                                    Clear
                                </button>
                            </Badge>
                        )}
                    </div>

                    <SearchPanel
                        onSearch={search}
                        onClear={clearSearch}
                        isSearching={isSearching(state)}
                        isAdmin={isAdmin}
                    />

                    {error && <Alert tone="error">{error}</Alert>}

                    {isLoadingList(state) ? (
                        <div className="space-y-3">
                            {Array.from({ length: 4 }, (_, index) => (
                                <Skeleton key={index} className="h-16 w-full" />
                            ))}
                        </div>
                    ) : state.items.length === 0 ? (
                        <EmptyState
                            icon={<SearchIcon className="h-6 w-6" />}
                            title={filters ? 'No matching links' : 'No links yet'}
                            description={
                                filters
                                    ? 'Try a different search term or adjust your filters.'
                                    : 'Create your first short link using the form above.'
                            }
                            action={
                                filters ? (
                                    <Button variant="secondary" size="sm" onClick={clearSearch}>
                                        Clear search
                                    </Button>
                                ) : undefined
                            }
                        />
                    ) : (
                        <>
                            {/* A status change refreshes the query in place, so toggling a
                                link does not discard the search that surfaced it. */}
                            <UrlList urls={state.items} isAdmin={isAdmin} onUrlsChanged={refresh} />
                            {hasMore ? (
                                <div
                                    ref={sentinelRef}
                                    className="flex min-h-10 items-center justify-center pt-2"
                                >
                                    {isPaging(state) && (
                                        <span className="inline-flex items-center gap-2 text-sm text-fg-muted">
                                            <Spinner className="h-4 w-4" />
                                            {isDraining(state)
                                                ? `Loading all… ${stats.count.toLocaleString()} loaded`
                                                : 'Loading more…'}
                                        </span>
                                    )}
                                </div>
                            ) : (
                                stats.count > PAGE_SIZE && (
                                    <p className="pt-2 text-center text-xs text-fg-subtle">
                                        You’ve reached the end · {stats.count.toLocaleString()} link
                                        {stats.count === 1 ? '' : 's'}
                                    </p>
                                )
                            )}
                        </>
                    )}
                </section>
            </PageShell>
        </div>
    );
};

export default Dashboard;
