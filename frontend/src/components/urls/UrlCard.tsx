import React from 'react';
import { Link } from 'react-router-dom';
import { BarChart3, ExternalLink } from 'lucide-react';
import type { ShortenedUrl } from '../../types';
import { encodeShortCodeForApi } from '../../utils/url';
import { formatDate, formatDateCompact } from '../../utils/date';
import { ANONYMOUS_PRINCIPAL } from '../../utils/identity';
import { cn } from '../../lib/cn';
import { Badge } from '../ui/Badge';
import { Button } from '../ui/Button';
import { CopyButton } from '../ui/CopyButton';
import { rowActionState, type UrlActionsController } from './urlActions';

export interface UrlCardProps {
    url: ShortenedUrl;
    /** Absolute short link, or `null` when no redirect base is configured. */
    shortLink: string | null;
    isAdmin: boolean;
    controller: UrlActionsController;
}

const FieldLabel: React.FC<{ children: React.ReactNode }> = ({ children }) => (
    <p className="text-xs font-medium uppercase tracking-wide text-fg-subtle">{children}</p>
);

/**
 * The narrow-viewport presentation. It renders the same action model as
 * `UrlTableRow` but labels the controls instead of reducing them to icons: a card has
 * the room, and a touch device cannot reveal a tooltip on hover — which is also why
 * the principal is shown in full here and abbreviated in the table.
 */
export const UrlCard: React.FC<UrlCardProps> = ({ url, shortLink, isAdmin, controller }) => {
    const detailsPath = `/url/${encodeShortCodeForApi(url.short_code)}`;
    const { spec, isRunning, disabled } = rowActionState(url, controller);
    const ActionIcon = spec.icon;

    return (
        <div className="space-y-4 rounded-2xl border border-border bg-surface p-4 shadow-soft">
            <div className="flex items-start justify-between gap-3">
                <Link
                    to={detailsPath}
                    className="min-w-0 break-all font-mono text-base font-semibold text-primary hover:underline"
                >
                    {url.short_code}
                </Link>
                <Badge tone={url.is_active ? 'success' : 'danger'} dot>
                    {url.is_active ? 'Active' : 'Inactive'}
                </Badge>
            </div>

            <div className="space-y-1.5">
                <FieldLabel>Destination</FieldLabel>
                <a
                    href={url.original_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    title={url.original_url}
                    className="inline-flex max-w-full items-start gap-1.5 break-all text-sm text-fg-muted hover:text-fg hover:underline"
                >
                    <span className="break-all">{url.original_url}</span>
                    <ExternalLink className="mt-0.5 h-3.5 w-3.5 shrink-0 opacity-60" />
                </a>
            </div>

            <div className="grid grid-cols-2 gap-3 rounded-xl border border-border/70 bg-surface-2/40 p-3">
                <div className="space-y-1">
                    <FieldLabel>Clicks</FieldLabel>
                    <p className="text-lg font-semibold tracking-tight text-fg">
                        {url.clicks.toLocaleString()}
                    </p>
                </div>
                <div className="space-y-1">
                    <FieldLabel>Created</FieldLabel>
                    <p className="text-sm font-medium text-fg-muted" title={formatDate(url.created_at)}>
                        {formatDateCompact(url.created_at)}
                    </p>
                </div>
                {isAdmin && (
                    <div className="col-span-2 space-y-1">
                        <FieldLabel>Created by</FieldLabel>
                        <p className="break-all font-mono text-sm text-fg-muted">
                            {url.created_by || ANONYMOUS_PRINCIPAL}
                        </p>
                    </div>
                )}
            </div>

            <div className="grid grid-cols-2 gap-2">
                {shortLink && (
                    <CopyButton
                        value={shortLink}
                        variant="secondary"
                        size="sm"
                        idleLabel="Copy link"
                        copiedLabel="Copied"
                        className="w-full"
                    />
                )}
                <Link
                    to={detailsPath}
                    className={cn(
                        'inline-flex h-8 items-center justify-center gap-2 rounded-lg border border-border px-2.5 text-xs font-medium text-fg transition-all duration-150 hover:border-primary hover:text-primary focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring sm:px-3',
                        shortLink ? '' : 'col-span-2',
                    )}
                >
                    <BarChart3 className="h-4 w-4" />
                    View analytics
                </Link>
                {isAdmin && (
                    <Button
                        variant={spec.tone}
                        size="sm"
                        fullWidth
                        className="col-span-2"
                        isLoading={isRunning}
                        disabled={disabled}
                        onClick={() => controller.request(url)}
                        leftIcon={!isRunning ? <ActionIcon className="h-4 w-4" /> : undefined}
                    >
                        {spec.label}
                    </Button>
                )}
            </div>
        </div>
    );
};
