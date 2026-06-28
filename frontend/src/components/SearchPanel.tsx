import React, { useCallback, useState } from 'react';
import { Search, SlidersHorizontal, X } from 'lucide-react';
import { cn } from '../lib/cn';
import { Button } from './ui/Button';
import { IconButton } from './ui/IconButton';
import { Field, Input, Select } from './ui/Input';

export interface SearchFilters {
    q: string;
    created_by?: string;
    created_from?: number;
    created_to?: number;
    is_active?: boolean;
}

interface SearchPanelProps {
    onSearch: (filters: SearchFilters) => void;
    onClear: () => void;
    isSearching: boolean;
    isAdmin: boolean;
}

const toStartOfDayUnix = (value: string): number | undefined => {
    if (!value) return undefined;
    const ms = new Date(`${value}T00:00:00`).getTime();
    return Number.isNaN(ms) ? undefined : Math.floor(ms / 1000);
};

const toEndOfDayUnix = (value: string): number | undefined => {
    if (!value) return undefined;
    // Exclusive upper bound: midnight of the following day.
    const ms = new Date(`${value}T00:00:00`).getTime() + 24 * 60 * 60 * 1000;
    return Number.isNaN(ms) ? undefined : Math.floor(ms / 1000);
};

const SearchPanel: React.FC<SearchPanelProps> = ({
    onSearch,
    onClear,
    isSearching,
    isAdmin,
}) => {
    const [query, setQuery] = useState('');
    const [showFilters, setShowFilters] = useState(false);
    const [createdBy, setCreatedBy] = useState('');
    const [createdFrom, setCreatedFrom] = useState('');
    const [createdTo, setCreatedTo] = useState('');
    const [status, setStatus] = useState<'all' | 'active' | 'inactive'>('all');

    const hasActiveFilters =
        createdBy.trim() !== '' || createdFrom !== '' || createdTo !== '' || status !== 'all';

    const buildFilters = useCallback(
        (q: string): SearchFilters => ({
            q,
            created_by: createdBy.trim() || undefined,
            created_from: toStartOfDayUnix(createdFrom),
            created_to: toEndOfDayUnix(createdTo),
            is_active: status === 'all' ? undefined : status === 'active',
        }),
        [createdBy, createdFrom, createdTo, status],
    );

    const handleSubmit = useCallback(
        (e: React.FormEvent) => {
            e.preventDefault();
            const trimmed = query.trim();
            if (trimmed) {
                onSearch(buildFilters(trimmed));
            }
        },
        [query, buildFilters, onSearch],
    );

    const handleClear = useCallback(() => {
        setQuery('');
        setCreatedBy('');
        setCreatedFrom('');
        setCreatedTo('');
        setStatus('all');
        onClear();
    }, [onClear]);

    const handleKeyDown = useCallback(
        (e: React.KeyboardEvent) => {
            if (e.key === 'Escape') {
                handleClear();
            }
        },
        [handleClear],
    );

    return (
        <form onSubmit={handleSubmit} className="space-y-2.5 sm:space-y-3">
            <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:gap-2.5">
                <div className="relative flex-1">
                    <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg-subtle sm:left-3.5 sm:h-4.5 sm:w-4.5" />
                    <Input
                        type="text"
                        value={query}
                        onChange={(e) => setQuery(e.target.value)}
                        onKeyDown={handleKeyDown}
                        placeholder="Search by short code or URL…"
                        className="pl-10 sm:pl-11"
                        aria-label="Search URLs"
                    />
                    {query && (
                        <button
                            type="button"
                            onClick={() => setQuery('')}
                            aria-label="Clear query"
                            className="absolute right-2 top-1/2 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-md text-fg-subtle transition-colors hover:bg-surface-2 hover:text-fg sm:right-2.5"
                        >
                            <X className="h-4 w-4" />
                        </button>
                    )}
                </div>

                <div className="grid grid-cols-2 gap-2 sm:flex sm:items-center">
                    <Button
                        type="button"
                        variant={showFilters || hasActiveFilters ? 'secondary' : 'outline'}
                        onClick={() => setShowFilters((s) => !s)}
                        leftIcon={<SlidersHorizontal className="h-4 w-4" />}
                        aria-expanded={showFilters}
                        className="w-full sm:w-auto"
                    >
                        Filters
                        {hasActiveFilters && (
                            <span className="ml-0.5 flex h-5 min-w-5 items-center justify-center rounded-full bg-primary px-1 text-xs font-semibold text-primary-fg">
                                •
                            </span>
                        )}
                    </Button>
                    <Button
                        type="submit"
                        isLoading={isSearching}
                        disabled={!query.trim()}
                        leftIcon={!isSearching ? <Search className="h-4 w-4" /> : undefined}
                        className="w-full sm:w-auto"
                    >
                        Search
                    </Button>
                </div>
            </div>

            <div
                className={cn(
                    'overflow-hidden transition-all duration-200 ease-out',
                    showFilters ? 'max-h-96 opacity-100' : 'max-h-0 opacity-0',
                )}
            >
                <div className="min-h-0 pt-0.5">
                    <div className="rounded-2xl border border-border bg-surface p-3.5 shadow-soft sm:p-5">
                        <div className="grid gap-3 sm:grid-cols-2 sm:gap-4 lg:grid-cols-4">
                            <Field label="Created from" htmlFor="created-from">
                                <Input
                                    id="created-from"
                                    type="date"
                                    value={createdFrom}
                                    onChange={(e) => setCreatedFrom(e.target.value)}
                                />
                            </Field>
                            <Field label="Created to" htmlFor="created-to">
                                <Input
                                    id="created-to"
                                    type="date"
                                    value={createdTo}
                                    onChange={(e) => setCreatedTo(e.target.value)}
                                />
                            </Field>
                            <Field label="Status" htmlFor="status-filter">
                                <Select
                                    id="status-filter"
                                    value={status}
                                    onChange={(e) => setStatus(e.target.value as 'all' | 'active' | 'inactive')}
                                >
                                    <option value="all">All statuses</option>
                                    <option value="active">Active</option>
                                    <option value="inactive">Inactive</option>
                                </Select>
                            </Field>
                            {isAdmin && (
                                <Field label="Created by" htmlFor="created-by">
                                    <Input
                                        id="created-by"
                                        type="text"
                                        value={createdBy}
                                        onChange={(e) => setCreatedBy(e.target.value)}
                                        placeholder="User ID"
                                    />
                                </Field>
                            )}
                        </div>
                        <div className="mt-3.5 flex flex-col gap-2.5 sm:mt-4 sm:flex-row sm:items-center sm:justify-between">
                            <p className="text-xs text-fg-subtle">
                                Filters apply together with your search query.
                            </p>
                            <IconButton label="Reset filters" onClick={handleClear} variant="outline" size="sm">
                                <X className="h-4 w-4" />
                            </IconButton>
                        </div>
                    </div>
                </div>
            </div>
        </form>
    );
};

export default SearchPanel;
