import React from 'react';
import { ArrowDownToLine, Link2, MousePointerClick, Signal } from 'lucide-react';
import type { CollectionStats } from '../urls/linkCollection';
import { Spinner } from '../ui/Spinner';
import { StatCard } from '../ui/StatCard';

export interface DashboardStatsProps {
    readonly stats: CollectionStats;
    /** Whether the counts describe a filtered result set or the whole list. */
    readonly filtered: boolean;
    readonly hasMore: boolean;
    readonly isDraining: boolean;
    readonly canLoadAll: boolean;
    readonly onLoadAll: () => void;
}

/**
 * The three summary cards.
 *
 * Pure in its props: it reads no context and issues no request, so what it displays is a
 * function of the collection alone and it can be reasoned about without the dashboard
 * around it.
 */
export const DashboardStats: React.FC<DashboardStatsProps> = ({
    stats,
    filtered,
    hasMore,
    isDraining,
    canLoadAll,
    onLoadAll,
}) => (
    <section className="grid gap-3 sm:grid-cols-2 sm:gap-4 lg:grid-cols-3">
        <StatCard
            label={filtered ? 'Links found' : 'Links loaded'}
            value={stats.count.toLocaleString()}
            icon={<Link2 className="h-5 w-5" />}
            tone="primary"
            className="h-full"
            hint={
                hasMore ? (
                    <button
                        type="button"
                        onClick={onLoadAll}
                        disabled={!canLoadAll}
                        className="inline-flex items-center gap-1 rounded font-medium text-primary underline-offset-2 transition hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:no-underline disabled:opacity-70"
                    >
                        {isDraining ? (
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
            className="h-full"
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
            className="h-full"
        />
    </section>
);
