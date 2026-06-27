import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
    ArrowLeft,
    CalendarDays,
    ExternalLink,
    History,
    MousePointerClick,
    Pencil,
    RotateCcw,
    Signal,
    UserRound,
} from 'lucide-react';
import {
    Bar,
    BarChart,
    Cell,
    Pie,
    PieChart,
    ResponsiveContainer,
    Tooltip,
    XAxis,
    YAxis,
} from 'recharts';
import { apiClient } from '../api';
import type { AnalyticsAggregate, AnalyticsEntry, ShortenedUrl, UrlHistoryEntry } from '../types';
import { buildShortLink, decodeShortCodeFromApi } from '../utils/url';
import { formatDate } from '../utils/date';
import { extractErrorMessage } from '../utils/errorHandling';
import { useTheme } from '../hooks/useTheme';
import { AppHeader } from './layout/AppHeader';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';
import { CopyButton } from './ui/CopyButton';
import { Alert } from './ui/Alert';
import { Dialog } from './ui/Dialog';
import { Field, Input } from './ui/Input';
import { EmptyState } from './ui/EmptyState';
import { Skeleton } from './ui/Skeleton';
import { StatCard } from './ui/StatCard';
import { SegmentedControl } from './ui/SegmentedControl';
import { Card, CardBody, CardHeader, CardTitle } from './ui/Card';
import { Table, TBody, TD, TH, THead, TR, TableScroll } from './ui/Table';

type AggregateDimension = 'country' | 'region' | 'city' | 'asn' | 'hour' | 'day';

const DIMENSIONS: { value: AggregateDimension; label: string }[] = [
    { value: 'country', label: 'Country' },
    { value: 'region', label: 'Region' },
    { value: 'city', label: 'City' },
    { value: 'asn', label: 'ASN' },
    { value: 'hour', label: 'Hour' },
    { value: 'day', label: 'Day' },
];

const DIMENSION_LABELS = Object.fromEntries(
    DIMENSIONS.map(({ value, label }) => [value, label]),
) as Record<AggregateDimension, string>;

const CHART_SERIES_TOKENS = [
    '--chart-1',
    '--chart-2',
    '--chart-3',
    '--chart-4',
    '--chart-5',
    '--chart-6',
    '--chart-7',
    '--chart-8',
    '--chart-9',
    '--chart-10',
];

interface ChartPalette {
    series: string[];
    other: string;
    axis: string;
    cursor: string;
}

/** Resolve chart colors from CSS tokens so index.css stays the single source. */
const readChartPalette = (): ChartPalette => {
    const styles = getComputedStyle(document.documentElement);
    const read = (token: string) => styles.getPropertyValue(token).trim();
    return {
        series: CHART_SERIES_TOKENS.map(read),
        other: read('--chart-other'),
        axis: read('--fg-subtle'),
        cursor: read('--chart-cursor'),
    };
};

const formatTimeBucket = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return `${date.toLocaleDateString()} ${date.toLocaleTimeString([], {
        hour: '2-digit',
        minute: '2-digit',
    })}`;
};

const formatDimensionValue = (value: string, dimension: AggregateDimension): string => {
    if (dimension === 'hour' || dimension === 'day') {
        const timestamp = parseInt(value, 10);
        if (!Number.isNaN(timestamp)) {
            const date = new Date(timestamp * 1000);
            return dimension === 'hour'
                ? date.toLocaleString([], {
                    month: 'short',
                    day: 'numeric',
                    hour: '2-digit',
                    minute: '2-digit',
                })
                : date.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' });
        }
    }
    return value || 'Unknown';
};

interface ChartDatum {
    name: string;
    value: number;
    isOther: boolean;
}

const ChartTooltip: React.FC<{
    active?: boolean;
    payload?: { payload: ChartDatum }[];
    total: number;
}> = ({ active, payload, total }) => {
    if (!active || !payload?.length) return null;
    const datum = payload[0].payload;
    const pct = total > 0 ? ((datum.value / total) * 100).toFixed(1) : '0.0';
    return (
        <div className="rounded-lg border border-border bg-elevated px-3 py-2 text-sm shadow-elevated">
            <p className="font-medium text-fg">{datum.name}</p>
            <p className="text-fg-muted">
                {datum.value.toLocaleString()} visits · {pct}%
            </p>
        </div>
    );
};

const UrlDetails: React.FC = () => {
    const { shortCode } = useParams<{ shortCode: string }>();
    const navigate = useNavigate();
    const { resolvedTheme } = useTheme();

    const [chartPalette, setChartPalette] = useState<ChartPalette>(readChartPalette);
    useEffect(() => {
        setChartPalette(readChartPalette());
    }, [resolvedTheme]);

    const [url, setUrl] = useState<ShortenedUrl | null>(null);
    const [analytics, setAnalytics] = useState<AnalyticsEntry[]>([]);
    const [aggregateStats, setAggregateStats] = useState<AnalyticsAggregate[]>([]);
    const [totalClicks, setTotalClicks] = useState<number>(0);
    const [selectedDimension, setSelectedDimension] = useState<AggregateDimension>('country');
    const [isLoadingUrl, setIsLoadingUrl] = useState(true);
    const [isLoadingAnalytics, setIsLoadingAnalytics] = useState(true);
    const [isLoadingAggregate, setIsLoadingAggregate] = useState(true);
    const [error, setError] = useState<string | null>(null);

    const [history, setHistory] = useState<UrlHistoryEntry[]>([]);
    const [isLoadingHistory, setIsLoadingHistory] = useState(true);
    const [historyError, setHistoryError] = useState<string | null>(null);

    const [isEditOpen, setIsEditOpen] = useState(false);
    const [editValue, setEditValue] = useState('');
    const [isSaving, setIsSaving] = useState(false);
    const [editError, setEditError] = useState<string | null>(null);

    const [restoreTarget, setRestoreTarget] = useState<UrlHistoryEntry | null>(null);
    const [isRestoring, setIsRestoring] = useState(false);
    const [restoreError, setRestoreError] = useState<string | null>(null);

    const decodedShortCode = useMemo(() => {
        if (!shortCode) return null;
        try {
            return decodeShortCodeFromApi(shortCode);
        } catch {
            return null;
        }
    }, [shortCode]);

    useEffect(() => {
        const loadUrlData = async () => {
            if (!shortCode) {
                navigate('/');
                return;
            }
            if (!decodedShortCode) {
                setError('Invalid short code');
                setIsLoadingUrl(false);
                return;
            }
            setIsLoadingUrl(true);
            setError(null);
            try {
                const urlData = await apiClient.getUrl(decodedShortCode);
                setUrl(urlData);
            } catch (err: unknown) {
                setError(extractErrorMessage(err, 'Failed to load URL details'));
            } finally {
                setIsLoadingUrl(false);
            }
        };
        loadUrlData();
    }, [shortCode, decodedShortCode, navigate]);

    const loadHistory = useCallback(async () => {
        if (!decodedShortCode) return;
        setIsLoadingHistory(true);
        setHistoryError(null);
        try {
            const data = await apiClient.getUrlHistory(decodedShortCode);
            setHistory(data);
        } catch (err: unknown) {
            setHistoryError(extractErrorMessage(err, 'Failed to load destination history'));
            setHistory([]);
        } finally {
            setIsLoadingHistory(false);
        }
    }, [decodedShortCode]);

    useEffect(() => {
        loadHistory();
    }, [loadHistory]);

    const openEdit = () => {
        setEditValue(url?.original_url ?? '');
        setEditError(null);
        setIsEditOpen(true);
    };

    const handleSaveEdit = async () => {
        if (!decodedShortCode) return;
        const trimmed = editValue.trim();
        if (!trimmed) {
            setEditError('Destination cannot be empty');
            return;
        }
        setIsSaving(true);
        setEditError(null);
        try {
            const updated = await apiClient.updateUrl(decodedShortCode, trimmed);
            setUrl(updated);
            setIsEditOpen(false);
            await loadHistory();
        } catch (err: unknown) {
            setEditError(extractErrorMessage(err, 'Failed to update destination'));
        } finally {
            setIsSaving(false);
        }
    };

    const handleRestore = async () => {
        if (!decodedShortCode || !restoreTarget) return;
        setIsRestoring(true);
        setRestoreError(null);
        try {
            const updated = await apiClient.restoreUrl(decodedShortCode, restoreTarget.id);
            setUrl(updated);
            setRestoreTarget(null);
            await loadHistory();
        } catch (err: unknown) {
            setRestoreError(extractErrorMessage(err, 'Failed to restore destination'));
        } finally {
            setIsRestoring(false);
        }
    };

    useEffect(() => {
        const loadAnalytics = async () => {
            if (!decodedShortCode) return;
            setIsLoadingAnalytics(true);
            try {
                const data = await apiClient.getAnalytics(decodedShortCode, undefined, undefined, 50);
                setAnalytics(data.entries);
                setTotalClicks(data.clicks);
            } catch (analyticsError) {
                console.warn('Analytics data not available:', analyticsError);
                setAnalytics([]);
                setTotalClicks(0);
            } finally {
                setIsLoadingAnalytics(false);
            }
        };
        loadAnalytics();
    }, [decodedShortCode]);

    useEffect(() => {
        const loadAggregate = async () => {
            if (!decodedShortCode) return;
            setIsLoadingAggregate(true);
            try {
                const data = await apiClient.getAnalyticsAggregate(
                    decodedShortCode,
                    selectedDimension,
                    undefined,
                    undefined,
                    20,
                );
                setAggregateStats(data.aggregates);
                setTotalClicks(data.clicks);
            } catch (aggregateError) {
                console.warn('Analytics aggregates not available:', aggregateError);
                setAggregateStats([]);
            } finally {
                setIsLoadingAggregate(false);
            }
        };
        loadAggregate();
    }, [decodedShortCode, selectedDimension]);

    const aggregatesWithOther = useMemo<AnalyticsAggregate[]>(() => {
        if (aggregateStats.length === 0 || totalClicks === 0) {
            return aggregateStats;
        }
        const accounted = aggregateStats.reduce((sum, s) => sum + s.visit_count, 0);
        const unaccounted = totalClicks - accounted;
        if (unaccounted > 0) {
            return [...aggregateStats, { dimension: 'Other', visit_count: unaccounted }];
        }
        return aggregateStats;
    }, [aggregateStats, totalClicks]);

    const chartData = useMemo<ChartDatum[]>(
        () =>
            aggregatesWithOther.slice(0, 10).map((stat) => ({
                name:
                    stat.dimension === 'Other'
                        ? 'Other'
                        : formatDimensionValue(stat.dimension, selectedDimension),
                value: stat.visit_count,
                isOther: stat.dimension === 'Other',
            })),
        [aggregatesWithOther, selectedDimension],
    );

    const shortLink = useMemo(
        () => (url ? buildShortLink(url.short_code, url.redirect_base_url) : null),
        [url],
    );

    if (error && !url) {
        return (
            <div className="min-h-screen bg-bg">
                <AppHeader />
                <main className="mx-auto max-w-6xl px-4 py-10 sm:px-6">
                    <Alert tone="error">
                        {error ||
                            (decodedShortCode ? `URL not found: ${decodedShortCode}` : 'Invalid short code')}
                    </Alert>
                    <Button
                        variant="secondary"
                        className="mt-6"
                        onClick={() => navigate('/')}
                        leftIcon={<ArrowLeft className="h-4 w-4" />}
                    >
                        Back to dashboard
                    </Button>
                </main>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-bg">
            <AppHeader />
            <main className="mx-auto max-w-6xl space-y-6 px-4 py-8 sm:px-6 sm:py-10">
                <div>
                    <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => navigate('/')}
                        leftIcon={<ArrowLeft className="h-4 w-4" />}
                        className="-ml-2 mb-3"
                    >
                        Back to dashboard
                    </Button>
                    <h1 className="text-2xl font-bold tracking-tight text-fg sm:text-3xl">
                        Link analytics
                    </h1>
                    <p className="mt-1 text-sm text-fg-muted">
                        Detailed performance and audience insights for your short link.
                    </p>
                </div>

                {/* Link information */}
                <Card>
                    <CardHeader>
                        <CardTitle>Link information</CardTitle>
                    </CardHeader>
                    <CardBody className="space-y-5">
                        {isLoadingUrl ? (
                            <div className="space-y-4">
                                <Skeleton className="h-14 w-full" />
                                <Skeleton className="h-14 w-full" />
                                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                                    {Array.from({ length: 4 }).map((_, i) => (
                                        <Skeleton key={i} className="h-20" />
                                    ))}
                                </div>
                            </div>
                        ) : url ? (
                            <>
                                <div className="space-y-1.5">
                                    <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">
                                        Short link
                                    </p>
                                    <div className="flex flex-col gap-3 sm:flex-row sm:items-center">
                                        <div className="min-w-0 flex-1 rounded-xl border border-border bg-surface-2/60 px-4 py-3">
                                            {shortLink ? (
                                                <a
                                                    href={shortLink}
                                                    target="_blank"
                                                    rel="noopener noreferrer"
                                                    className="inline-flex max-w-full items-center gap-1.5 break-all font-medium text-primary hover:underline"
                                                >
                                                    <span className="break-all">{shortLink}</span>
                                                    <ExternalLink className="h-3.5 w-3.5 shrink-0 opacity-60" />
                                                </a>
                                            ) : (
                                                <span className="break-all font-mono text-sm text-fg">
                                                    {url.short_code}
                                                </span>
                                            )}
                                        </div>
                                        {(shortLink || url.short_code) && (
                                            <CopyButton
                                                value={shortLink ?? url.short_code}
                                                variant="secondary"
                                                size="md"
                                                idleLabel="Copy link"
                                            />
                                        )}
                                    </div>
                                </div>

                                <div className="space-y-1.5">
                                    <div className="flex items-center justify-between gap-3">
                                        <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">
                                            Destination
                                        </p>
                                        <Button
                                            variant="ghost"
                                            size="sm"
                                            onClick={openEdit}
                                            leftIcon={<Pencil className="h-3.5 w-3.5" />}
                                        >
                                            Edit
                                        </Button>
                                    </div>
                                    <div className="rounded-xl border border-border bg-surface-2/60 px-4 py-3">
                                        <a
                                            href={url.original_url}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            className="break-all text-sm text-fg-muted hover:text-fg hover:underline"
                                        >
                                            {url.original_url}
                                        </a>
                                    </div>
                                </div>

                                <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
                                    <StatCard
                                        label="Total clicks"
                                        value={url.clicks.toLocaleString()}
                                        icon={<MousePointerClick className="h-5 w-5" />}
                                        tone="primary"
                                    />
                                    <StatCard
                                        label="Status"
                                        value={
                                            <Badge tone={url.is_active ? 'success' : 'danger'} dot>
                                                {url.is_active ? 'Active' : 'Inactive'}
                                            </Badge>
                                        }
                                        icon={<Signal className="h-5 w-5" />}
                                        tone={url.is_active ? 'success' : 'neutral'}
                                    />
                                    <StatCard
                                        label="Created"
                                        value={
                                            <span className="text-sm font-medium">{formatDate(url.created_at)}</span>
                                        }
                                        icon={<CalendarDays className="h-5 w-5" />}
                                        tone="neutral"
                                    />
                                    <StatCard
                                        label="Created by"
                                        value={<span className="text-base">{url.created_by || '—'}</span>}
                                        icon={<UserRound className="h-5 w-5" />}
                                        tone="accent"
                                    />
                                </div>
                            </>
                        ) : null}
                    </CardBody>
                </Card>

                {/* Destination history */}
                <Card>
                    <CardHeader>
                        <CardTitle>Destination history</CardTitle>
                    </CardHeader>
                    <CardBody>
                        {isLoadingHistory ? (
                            <div className="space-y-3">
                                <Skeleton className="h-12 w-full" />
                                <Skeleton className="h-12 w-full" />
                            </div>
                        ) : historyError ? (
                            <Alert tone="error">{historyError}</Alert>
                        ) : history.length === 0 ? (
                            <EmptyState
                                icon={<History className="h-6 w-6" />}
                                title="No previous destinations"
                                description="When you change this link's destination, the previous values appear here so you can restore them."
                            />
                        ) : (
                            <TableScroll>
                                <Table>
                                    <THead>
                                        <TR>
                                            <TH>Previous destination</TH>
                                            <TH>Changed</TH>
                                            <TH>Changed by</TH>
                                            <TH className="text-right">Action</TH>
                                        </TR>
                                    </THead>
                                    <TBody>
                                        {history.map((entry) => (
                                            <TR key={entry.id}>
                                                <TD>
                                                    <a
                                                        href={entry.historic_url}
                                                        target="_blank"
                                                        rel="noopener noreferrer"
                                                        className="break-all text-sm text-fg-muted hover:text-fg hover:underline"
                                                    >
                                                        {entry.historic_url}
                                                    </a>
                                                </TD>
                                                <TD className="whitespace-nowrap text-fg-muted">
                                                    {formatDate(entry.changed_at)}
                                                </TD>
                                                <TD className="text-fg-muted">{entry.changed_by || '—'}</TD>
                                                <TD className="text-right">
                                                    <Button
                                                        variant="secondary"
                                                        size="sm"
                                                        onClick={() => {
                                                            setRestoreError(null);
                                                            setRestoreTarget(entry);
                                                        }}
                                                        leftIcon={<RotateCcw className="h-3.5 w-3.5" />}
                                                    >
                                                        Restore
                                                    </Button>
                                                </TD>
                                            </TR>
                                        ))}
                                    </TBody>
                                </Table>
                            </TableScroll>
                        )}
                    </CardBody>
                </Card>

                {/* Analytics by dimension */}
                <Card>
                    <CardHeader className="flex-row flex-wrap items-center justify-between gap-3">
                        <CardTitle>Analytics by dimension</CardTitle>
                        <SegmentedControl
                            ariaLabel="Group analytics by"
                            options={DIMENSIONS}
                            value={selectedDimension}
                            onChange={setSelectedDimension}
                        />
                    </CardHeader>
                    <CardBody>
                        {isLoadingAggregate ? (
                            <Skeleton className="h-72 w-full" />
                        ) : aggregatesWithOther.length > 0 ? (
                            <div className="space-y-6">
                                <div className="grid gap-6 lg:grid-cols-5">
                                    <div className="lg:col-span-3">
                                        <p className="mb-3 text-xs font-medium uppercase tracking-wide text-fg-subtle">
                                            Top {DIMENSION_LABELS[selectedDimension].toLowerCase()} by visits
                                        </p>
                                        <div className="h-72 w-full">
                                            <ResponsiveContainer width="100%" height="100%">
                                                <BarChart
                                                    data={chartData}
                                                    layout="vertical"
                                                    margin={{ top: 0, right: 16, left: 0, bottom: 0 }}
                                                >
                                                    <XAxis
                                                        type="number"
                                                        tick={{ fill: chartPalette.axis, fontSize: 12 }}
                                                        axisLine={false}
                                                        tickLine={false}
                                                    />
                                                    <YAxis
                                                        type="category"
                                                        dataKey="name"
                                                        width={110}
                                                        tick={{ fill: chartPalette.axis, fontSize: 12 }}
                                                        axisLine={false}
                                                        tickLine={false}
                                                        tickFormatter={(value: string) =>
                                                            value.length > 16 ? `${value.slice(0, 15)}…` : value
                                                        }
                                                    />
                                                    <Tooltip
                                                        cursor={{ fill: chartPalette.cursor }}
                                                        content={<ChartTooltip total={totalClicks} />}
                                                    />
                                                    <Bar dataKey="value" radius={[0, 6, 6, 0]} maxBarSize={26}>
                                                        {chartData.map((entry, index) => (
                                                            <Cell
                                                                key={entry.name}
                                                                fill={
                                                                    entry.isOther
                                                                        ? chartPalette.other
                                                                        : chartPalette.series[
                                                                              index % chartPalette.series.length
                                                                          ]
                                                                }
                                                            />
                                                        ))}
                                                    </Bar>
                                                </BarChart>
                                            </ResponsiveContainer>
                                        </div>
                                    </div>

                                    <div className="lg:col-span-2">
                                        <p className="mb-3 text-xs font-medium uppercase tracking-wide text-fg-subtle">
                                            Share of total
                                        </p>
                                        <div className="h-72 w-full">
                                            <ResponsiveContainer width="100%" height="100%">
                                                <PieChart>
                                                    <Pie
                                                        data={chartData}
                                                        dataKey="value"
                                                        nameKey="name"
                                                        innerRadius="55%"
                                                        outerRadius="80%"
                                                        paddingAngle={2}
                                                        stroke="none"
                                                    >
                                                        {chartData.map((entry, index) => (
                                                            <Cell
                                                                key={entry.name}
                                                                fill={
                                                                    entry.isOther
                                                                        ? chartPalette.other
                                                                        : chartPalette.series[
                                                                              index % chartPalette.series.length
                                                                          ]
                                                                }
                                                            />
                                                        ))}
                                                    </Pie>
                                                    <Tooltip content={<ChartTooltip total={totalClicks} />} />
                                                </PieChart>
                                            </ResponsiveContainer>
                                        </div>
                                    </div>
                                </div>

                                <TableScroll className="shadow-none">
                                    <Table>
                                        <THead>
                                            <TR className="border-b-0">
                                                <TH>{DIMENSION_LABELS[selectedDimension]}</TH>
                                                <TH className="text-right">Visits</TH>
                                                <TH className="w-1/2">Distribution</TH>
                                            </TR>
                                        </THead>
                                        <TBody>
                                            {aggregatesWithOther.map((stat, index) => {
                                                const percentage =
                                                    totalClicks > 0 ? (stat.visit_count / totalClicks) * 100 : 0;
                                                const isOther = stat.dimension === 'Other';
                                                return (
                                                    <TR key={`${stat.dimension}-${index}`}>
                                                        <TD
                                                            className={
                                                                isOther ? 'italic text-fg-muted' : 'font-medium text-fg'
                                                            }
                                                        >
                                                            {isOther
                                                                ? stat.dimension
                                                                : formatDimensionValue(stat.dimension, selectedDimension)}
                                                        </TD>
                                                        <TD className="text-right font-medium tabular-nums">
                                                            {stat.visit_count.toLocaleString()}
                                                        </TD>
                                                        <TD>
                                                            <div className="flex items-center gap-3">
                                                                <div className="h-2 flex-1 overflow-hidden rounded-full bg-surface-2">
                                                                    <div
                                                                        className={
                                                                            isOther
                                                                                ? 'h-full rounded-full bg-fg-subtle'
                                                                                : 'h-full rounded-full bg-primary'
                                                                        }
                                                                        style={{ width: `${percentage}%` }}
                                                                    />
                                                                </div>
                                                                <span className="w-12 text-right text-[13px] text-fg-subtle tabular-nums">
                                                                    {percentage.toFixed(1)}%
                                                                </span>
                                                            </div>
                                                        </TD>
                                                    </TR>
                                                );
                                            })}
                                        </TBody>
                                    </Table>
                                </TableScroll>
                            </div>
                        ) : (
                            <EmptyState
                                title={`No ${DIMENSION_LABELS[selectedDimension].toLowerCase()} data yet`}
                                description="Analytics will appear here once your link starts receiving visits."
                            />
                        )}
                    </CardBody>
                </Card>

                {/* Recent activity */}
                <Card>
                    <CardHeader>
                        <CardTitle>Recent activity</CardTitle>
                    </CardHeader>
                    <CardBody>
                        {isLoadingAnalytics ? (
                            <Skeleton className="h-64 w-full" />
                        ) : analytics.length > 0 ? (
                            <TableScroll className="shadow-none">
                                <Table>
                                    <THead>
                                        <TR className="border-b-0">
                                            <TH>Time period</TH>
                                            <TH>Country</TH>
                                            <TH>Region</TH>
                                            <TH>City</TH>
                                            <TH className="text-right">Visits</TH>
                                        </TR>
                                    </THead>
                                    <TBody>
                                        {analytics.slice(0, 20).map((entry) => (
                                            <TR key={entry.id}>
                                                <TD className="whitespace-nowrap">{formatTimeBucket(entry.time_bucket)}</TD>
                                                <TD className="text-fg-muted">{entry.country_code || 'N/A'}</TD>
                                                <TD className="text-fg-muted">{entry.region || 'N/A'}</TD>
                                                <TD className="text-fg-muted">{entry.city || 'N/A'}</TD>
                                                <TD className="text-right font-medium tabular-nums">
                                                    {entry.visit_count.toLocaleString()}
                                                </TD>
                                            </TR>
                                        ))}
                                    </TBody>
                                </Table>
                            </TableScroll>
                        ) : (
                            <EmptyState
                                title="No recent activity"
                                description="Analytics will appear once your link receives visits."
                            />
                        )}
                    </CardBody>
                </Card>
            </main>

            <Dialog
                open={isEditOpen}
                onClose={() => {
                    if (!isSaving) setIsEditOpen(false);
                }}
                title="Edit destination"
                description="Update where this short link points. The previous destination is saved to history."
                footer={
                    <>
                        <Button
                            variant="secondary"
                            onClick={() => setIsEditOpen(false)}
                            disabled={isSaving}
                        >
                            Cancel
                        </Button>
                        <Button
                            onClick={handleSaveEdit}
                            isLoading={isSaving}
                            disabled={editValue.trim().length === 0}
                        >
                            Save destination
                        </Button>
                    </>
                }
            >
                <div className="space-y-4">
                    <Field label="Destination URL" htmlFor="edit-destination">
                        <Input
                            id="edit-destination"
                            value={editValue}
                            onChange={(event) => setEditValue(event.target.value)}
                            placeholder="https://example.com/page"
                            invalid={Boolean(editError)}
                            autoFocus
                            onKeyDown={(event) => {
                                if (event.key === 'Enter') {
                                    event.preventDefault();
                                    handleSaveEdit();
                                }
                            }}
                        />
                    </Field>
                    {editError && <Alert tone="error">{editError}</Alert>}
                </div>
            </Dialog>

            <Dialog
                open={restoreTarget !== null}
                onClose={() => {
                    if (!isRestoring) setRestoreTarget(null);
                }}
                title="Restore destination"
                description="This makes the selected previous destination the link's active target. The current destination is saved to history."
                footer={
                    <>
                        <Button
                            variant="secondary"
                            onClick={() => setRestoreTarget(null)}
                            disabled={isRestoring}
                        >
                            Cancel
                        </Button>
                        <Button onClick={handleRestore} isLoading={isRestoring}>
                            Restore
                        </Button>
                    </>
                }
            >
                <div className="space-y-4">
                    <div className="rounded-xl border border-border bg-surface-2/60 px-4 py-3">
                        <p className="mb-1 text-xs font-medium uppercase tracking-wide text-fg-subtle">
                            Restore to
                        </p>
                        <p className="break-all text-sm text-fg">{restoreTarget?.historic_url}</p>
                    </div>
                    {restoreError && <Alert tone="error">{restoreError}</Alert>}
                </div>
            </Dialog>
        </div>
    );
};

export default UrlDetails;
