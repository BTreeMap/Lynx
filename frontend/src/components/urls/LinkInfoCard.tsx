import React from 'react';
import {
    CalendarDays,
    ExternalLink,
    MousePointerClick,
    Pencil,
    Signal,
    UserRound,
} from 'lucide-react';
import type { ShortenedUrl } from '../../types';
import { buildShortLink } from '../../utils/url';
import { formatDate } from '../../utils/date';
import { ANONYMOUS_PRINCIPAL } from '../../utils/identity';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { Card, CardBody, CardSectionHeader, CardTitle } from '../ui/Card';
import { CopyButton } from '../ui/CopyButton';
import { Skeleton } from '../ui/Skeleton';
import { StatCard } from '../ui/StatCard';

const FieldLabel: React.FC<{ readonly children: React.ReactNode }> = ({ children }) => (
    <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">{children}</p>
);

const LoadingBody: React.FC = () => (
    <div className="space-y-3 sm:space-y-4">
        <Skeleton className="h-14 w-full" />
        <Skeleton className="h-14 w-full" />
        <div className="grid gap-3 sm:grid-cols-2 sm:gap-4 lg:grid-cols-4">
            {Array.from({ length: 4 }, (_, index) => (
                <Skeleton key={index} className="h-20" />
            ))}
        </div>
    </div>
);

export interface LinkInfoCardProps {
    /** `null` while the link is still loading. */
    readonly url: ShortenedUrl | null;
    readonly onEdit: (currentDestination: string) => void;
}

/** Identity, destination and headline figures for one link. */
export const LinkInfoCard: React.FC<LinkInfoCardProps> = ({ url, onEdit }) => {
    const shortLink = url ? buildShortLink(url.short_code, url.redirect_base_url) : null;

    return (
        <Card>
            <CardSectionHeader>
                <CardTitle>Link information</CardTitle>
            </CardSectionHeader>
            <CardBody className="space-y-4 sm:space-y-5">
                {url === null ? (
                    <LoadingBody />
                ) : (
                    <>
                        <div className="space-y-1">
                            <FieldLabel>Short link</FieldLabel>
                            <div className="flex flex-col gap-2.5 sm:flex-row sm:items-center sm:gap-3">
                                <div className="min-w-0 flex-1 rounded-xl border border-border bg-surface-2/60 px-4 py-3">
                                    {shortLink ? (
                                        <a
                                            href={shortLink}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            className="inline-flex max-w-full items-start gap-1.5 break-all font-medium text-primary hover:underline"
                                        >
                                            <span className="break-all leading-snug">
                                                {shortLink}
                                            </span>
                                            <ExternalLink className="mt-0.5 h-3.5 w-3.5 shrink-0 opacity-60" />
                                        </a>
                                    ) : (
                                        <span className="break-all font-mono text-sm text-fg">
                                            {url.short_code}
                                        </span>
                                    )}
                                </div>
                                <CopyButton
                                    value={shortLink ?? url.short_code}
                                    variant="secondary"
                                    size="md"
                                    idleLabel="Copy link"
                                    className="w-full sm:w-auto"
                                />
                            </div>
                        </div>

                        <div className="space-y-1">
                            <div className="flex items-center justify-between gap-3">
                                <FieldLabel>Destination</FieldLabel>
                                <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => onEdit(url.original_url)}
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
                                    className="block break-all text-sm text-fg-muted hover:text-fg hover:underline"
                                >
                                    {url.original_url}
                                </a>
                            </div>
                        </div>

                        <div className="grid gap-3 sm:grid-cols-2 sm:gap-4 xl:grid-cols-4">
                            <StatCard
                                label="Total clicks"
                                value={url.clicks.toLocaleString()}
                                icon={<MousePointerClick className="h-5 w-5" />}
                                tone="primary"
                                className="h-full"
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
                                valueClassName="min-h-8 text-base font-medium sm:min-h-10 sm:text-lg"
                                className="h-full"
                            />
                            <StatCard
                                label="Created"
                                value={
                                    <span className="text-sm font-medium leading-snug text-fg sm:text-base">
                                        {formatDate(url.created_at)}
                                    </span>
                                }
                                icon={<CalendarDays className="h-5 w-5" />}
                                tone="neutral"
                                valueClassName="min-h-8 items-start text-base sm:min-h-10 sm:text-lg"
                                className="h-full"
                            />
                            <StatCard
                                label="Created by"
                                value={
                                    <span className="block break-all font-mono text-sm font-medium leading-relaxed text-fg-muted sm:text-base">
                                        {url.created_by || ANONYMOUS_PRINCIPAL}
                                    </span>
                                }
                                icon={<UserRound className="h-5 w-5" />}
                                tone="accent"
                                valueClassName="min-h-0 items-start text-base sm:text-lg"
                                className="h-full sm:col-span-2 xl:col-span-1"
                            />
                        </div>
                    </>
                )}
            </CardBody>
        </Card>
    );
};
