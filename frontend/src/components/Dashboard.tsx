import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowDownToLine, Download, Link2, MousePointerClick, Search as SearchIcon, Signal } from 'lucide-react';
import { useAuth } from '../hooks/useAuth';
import { apiClient } from '../api';
import { extractErrorMessage } from '../utils/errorHandling';
import type { ShortenedUrl } from '../types';
import CreateUrlForm from './CreateUrlForm';
import SearchPanel, { type SearchFilters } from './SearchPanel';
import UrlList from './UrlList';
import { AppHeader } from './layout/AppHeader';
import { Button } from './ui/Button';
import { Alert } from './ui/Alert';
import { EmptyState } from './ui/EmptyState';
import { Skeleton } from './ui/Skeleton';
import { Spinner } from './ui/Spinner';
import { StatCard } from './ui/StatCard';
import { Badge } from './ui/Badge';

const PAGE_SIZE = 50;
const PACE_MS = 200;
const supportsIntersectionObserver =
    typeof window !== 'undefined' && 'IntersectionObserver' in window;
const delay = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

const Dashboard: React.FC = () => {
    const { userInfo } = useAuth();
    const isAdmin = userInfo?.is_admin ?? false;

    const [urls, setUrls] = useState<ShortenedUrl[]>([]);
    const [isLoading, setIsLoading] = useState(true);
    const [isLoadingMore, setIsLoadingMore] = useState(false);
    const [isLoadingAll, setIsLoadingAll] = useState(false);
    const [isSearching, setIsSearching] = useState(false);
    const [isExporting, setIsExporting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const [nextCursor, setNextCursor] = useState<string | null>(null);
    const [hasMore, setHasMore] = useState(false);
    const [activeFilters, setActiveFilters] = useState<SearchFilters | null>(null);
    const sentinelRef = useRef<HTMLDivElement | null>(null);

    const loadUrls = useCallback(async () => {
        setIsLoading(true);
        setActiveFilters(null);
        setError(null);
        try {
            const data = await apiClient.listUrls(PAGE_SIZE);
            setUrls(data.urls);
            setNextCursor(data.next_cursor || null);
            setHasMore(data.has_more);
        } catch (err: unknown) {
            setError(extractErrorMessage(err, 'Failed to load URLs'));
            setUrls([]);
        } finally {
            setIsLoading(false);
        }
    }, []);

    const loadMoreUrls = useCallback(async () => {
        if (!nextCursor || isLoadingMore || isLoadingAll) return;
        setIsLoadingMore(true);
        setError(null);
        try {
            if (activeFilters) {
                const data = await apiClient.searchUrls({
                    ...activeFilters,
                    limit: PAGE_SIZE,
                    cursor: nextCursor,
                });
                setUrls((prev) => [...prev, ...data.items]);
                setNextCursor(data.next_cursor || null);
                setHasMore(data.has_more);
            } else {
                const data = await apiClient.listUrls(PAGE_SIZE, nextCursor);
                setUrls((prev) => [...prev, ...data.urls]);
                setNextCursor(data.next_cursor || null);
                setHasMore(data.has_more);
            }
        } catch (err: unknown) {
            setError(extractErrorMessage(err, 'Failed to load more URLs'));
        } finally {
            setIsLoadingMore(false);
        }
    }, [nextCursor, isLoadingMore, isLoadingAll, activeFilters]);

    const loadAllRemaining = useCallback(async () => {
        if (!nextCursor || isLoadingMore || isLoadingAll) return;
        setIsLoadingAll(true);
        setError(null);
        try {
            let cursor: string | null = nextCursor;
            while (cursor) {
                if (activeFilters) {
                    const data = await apiClient.searchUrls({
                        ...activeFilters,
                        limit: PAGE_SIZE,
                        cursor,
                    });
                    setUrls((prev) => [...prev, ...data.items]);
                    cursor = data.next_cursor || null;
                    setNextCursor(cursor);
                    setHasMore(data.has_more);
                } else {
                    const data = await apiClient.listUrls(PAGE_SIZE, cursor);
                    setUrls((prev) => [...prev, ...data.urls]);
                    cursor = data.next_cursor || null;
                    setNextCursor(cursor);
                    setHasMore(data.has_more);
                }
                if (cursor) await delay(PACE_MS);
            }
        } catch (err: unknown) {
            setError(extractErrorMessage(err, 'Failed to load all URLs'));
        } finally {
            setIsLoadingAll(false);
        }
    }, [nextCursor, isLoadingMore, isLoadingAll, activeFilters]);

    const handleSearch = useCallback(async (filters: SearchFilters) => {
        setIsSearching(true);
        setError(null);
        setActiveFilters(filters);
        setUrls([]);
        setNextCursor(null);
        try {
            const data = await apiClient.searchUrls({ ...filters, limit: PAGE_SIZE });
            setUrls(data.items);
            setNextCursor(data.next_cursor || null);
            setHasMore(data.has_more);
        } catch (err: unknown) {
            setError(extractErrorMessage(err, 'Search failed'));
        } finally {
            setIsSearching(false);
        }
    }, []);

    const handleClearSearch = useCallback(() => {
        loadUrls();
    }, [loadUrls]);

    const exportToJson = useCallback(async () => {
        setIsExporting(true);
        setError(null);
        try {
            const allUrls: ShortenedUrl[] = [];
            let cursor: string | null = null;
            let hasMoreData = true;
            while (hasMoreData) {
                const data = await apiClient.listUrls(PAGE_SIZE, cursor || undefined);
                allUrls.push(...data.urls);
                cursor = data.next_cursor || null;
                hasMoreData = data.has_more;
            }
            const jsonStr = JSON.stringify(allUrls, null, 2);
            const blob = new Blob([jsonStr], { type: 'application/json' });
            const objectUrl = URL.createObjectURL(blob);
            const link = document.createElement('a');
            link.href = objectUrl;
            link.download = `lynx-urls-export-${new Date().toISOString().split('T')[0]}.json`;
            document.body.appendChild(link);
            link.click();
            document.body.removeChild(link);
            URL.revokeObjectURL(objectUrl);
        } catch (err: unknown) {
            setError(extractErrorMessage(err, 'Failed to export URLs'));
        } finally {
            setIsExporting(false);
        }
    }, []);

    useEffect(() => {
        loadUrls();
    }, [loadUrls]);

    useEffect(() => {
        if (!supportsIntersectionObserver) return;
        const node = sentinelRef.current;
        if (!node || !hasMore) return;
        const observer = new IntersectionObserver(
            (entries) => {
                if (entries[0]?.isIntersecting) {
                    loadMoreUrls();
                }
            },
            { rootMargin: '320px 0px' },
        );
        observer.observe(node);
        return () => observer.disconnect();
    }, [hasMore, loadMoreUrls]);

    const stats = useMemo(() => {
        const totalClicks = urls.reduce((sum, u) => sum + u.clicks, 0);
        const active = urls.filter((u) => u.is_active).length;
        return {
            count: urls.length,
            totalClicks,
            active,
            inactive: urls.length - active,
        };
    }, [urls]);

    return (
        <div className="min-h-screen bg-bg">
            <AppHeader
                actions={
                    <Button
                        variant="secondary"
                        size="sm"
                        onClick={exportToJson}
                        isLoading={isExporting}
                        leftIcon={!isExporting ? <Download className="h-4 w-4" /> : undefined}
                    >
                        <span className="hidden sm:inline">Export JSON</span>
                        <span className="sm:hidden">Export</span>
                    </Button>
                }
            />

            <main className="mx-auto max-w-6xl space-y-6 px-3 py-6 sm:space-y-8 sm:px-6 sm:py-10">
                <section>
                    <h1 className="text-xl font-bold tracking-tight text-fg sm:text-3xl">Dashboard</h1>
                    <p className="mt-1 text-sm text-fg-muted">
                        Create, manage, and track your short links.
                    </p>
                </section>

                <section className="grid gap-3 sm:grid-cols-2 sm:gap-4 lg:grid-cols-3">
                    <StatCard
                        label={activeFilters ? 'Links found' : 'Links loaded'}
                        value={stats.count.toLocaleString()}
                        icon={<Link2 className="h-5 w-5" />}
                        tone="primary"
                        hint={
                            hasMore ? (
                                <button
                                    type="button"
                                    onClick={loadAllRemaining}
                                    disabled={isLoadingAll || isLoadingMore}
                                    className="inline-flex items-center gap-1 rounded font-medium text-primary underline-offset-2 transition hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:no-underline disabled:opacity-70"
                                >
                                    {isLoadingAll ? (
                                        <>
                                            <Spinner className="h-3 w-3" />
                                            Loading all… {stats.count.toLocaleString()} loaded
                                        </>
                                    ) : (
                                        <>
                                            <ArrowDownToLine className="h-3 w-3" />
                                            Load all
                                        </>
                                    )}
                                </button>
                            ) : undefined
                        }
                    />
                    <StatCard
                        label="Clicks (shown)"
                        value={stats.totalClicks.toLocaleString()}
                        icon={<MousePointerClick className="h-5 w-5" />}
                        tone="accent"
                    />
                    <StatCard
                        label="Active / Inactive"
                        value={
                            <span className="flex items-baseline gap-2">
                                {stats.active.toLocaleString()}
                                <span className="text-base font-normal text-fg-subtle">
                                    / {stats.inactive.toLocaleString()}
                                </span>
                            </span>
                        }
                        icon={<Signal className="h-5 w-5" />}
                        tone="success"
                    />
                </section>

                <CreateUrlForm onUrlCreated={loadUrls} />

                <section className="space-y-3 sm:space-y-4">
                    <div className="flex flex-wrap items-end justify-between gap-2.5 sm:gap-3">
                        <div>
                            <h2 className="text-lg font-semibold tracking-tight text-fg">Your links</h2>
                            <p className="text-sm text-fg-muted">
                                {activeFilters
                                    ? `Showing results for “${activeFilters.q}”`
                                    : 'All links you have created.'}
                            </p>
                        </div>
                        {activeFilters && (
                            <Badge tone="primary" className="gap-2 py-1 pl-2.5 pr-1.5">
                                {stats.count}
                                {hasMore ? '+' : ''} result{stats.count === 1 ? '' : 's'}
                                <button
                                    type="button"
                                    onClick={handleClearSearch}
                                    className="rounded-full px-2 py-0.5 text-xs font-medium text-primary-soft-fg/80 underline-offset-2 hover:underline"
                                >
                                    Clear
                                </button>
                            </Badge>
                        )}
                    </div>

                    <SearchPanel
                        onSearch={handleSearch}
                        onClear={handleClearSearch}
                        isSearching={isSearching}
                        isAdmin={isAdmin}
                    />

                    {error && <Alert tone="error">{error}</Alert>}

                    {isLoading ? (
                        <div className="space-y-3">
                            {Array.from({ length: 4 }).map((_, i) => (
                                <Skeleton key={i} className="h-16 w-full" />
                            ))}
                        </div>
                    ) : urls.length === 0 ? (
                        <EmptyState
                            icon={<SearchIcon className="h-6 w-6" />}
                            title={activeFilters ? 'No matching links' : 'No links yet'}
                            description={
                                activeFilters
                                    ? 'Try a different search term or adjust your filters.'
                                    : 'Create your first short link using the form above.'
                            }
                            action={
                                activeFilters ? (
                                    <Button variant="secondary" size="sm" onClick={handleClearSearch}>
                                        Clear search
                                    </Button>
                                ) : undefined
                            }
                        />
                    ) : (
                        <>
                            <UrlList urls={urls} isAdmin={isAdmin} onUrlsChanged={loadUrls} />
                            {hasMore ? (
                                supportsIntersectionObserver ? (
                                    <div
                                        ref={sentinelRef}
                                        className="flex min-h-10 items-center justify-center pt-2"
                                    >
                                        {(isLoadingMore || isLoadingAll) && (
                                            <span className="inline-flex items-center gap-2 text-sm text-fg-muted">
                                                <Spinner className="h-4 w-4" />
                                                {isLoadingAll
                                                    ? `Loading all… ${stats.count.toLocaleString()} loaded`
                                                    : 'Loading more…'}
                                            </span>
                                        )}
                                    </div>
                                ) : (
                                    <div className="flex justify-center pt-2">
                                        <Button
                                            variant="secondary"
                                            onClick={loadMoreUrls}
                                            isLoading={isLoadingMore}
                                        >
                                            {isLoadingMore ? 'Loading…' : 'Load more'}
                                        </Button>
                                    </div>
                                )
                            ) : (
                                urls.length > PAGE_SIZE && (
                                    <p className="pt-2 text-center text-xs text-fg-subtle">
                                        You’ve reached the end · {stats.count.toLocaleString()} link
                                        {stats.count === 1 ? '' : 's'}
                                    </p>
                                )
                            )}
                        </>
                    )}
                </section>
            </main>
        </div>
    );
};

export default Dashboard;
