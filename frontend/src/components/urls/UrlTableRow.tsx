import React from 'react';
import { Link } from 'react-router-dom';
import { BarChart3, ExternalLink } from 'lucide-react';
import type { ShortenedUrl } from '../../types';
import { encodeShortCodeForApi } from '../../utils/url';
import { formatDate, formatDateCompact } from '../../utils/date';
import { ANONYMOUS_PRINCIPAL, shortenPrincipal } from '../../utils/identity';
import { cn } from '../../lib/cn';
import { Badge } from '../ui/Badge';
import { CopyButton } from '../ui/CopyButton';
import { IconButton } from '../ui/IconButton';
import { iconButtonClass } from '../ui/iconButtonStyles';
import { TD } from '../ui/Table';
import { ROW_HOVER, stickyBodyCell } from '../ui/tableStyles';
import { rowActionState, type UrlActionsController } from './urlActions';

export interface UrlTableRowProps {
    url: ShortenedUrl;
    /** Absolute short link, or `null` when no redirect base is configured. */
    shortLink: string | null;
    isAdmin: boolean;
    controller: UrlActionsController;
    /** Virtualiser bookkeeping — the measured element must be the `<tr>` itself. */
    index: number;
    measureRef: (node: HTMLTableRowElement | null) => void;
}

/**
 * One desktop row.
 *
 * Cell order is load-bearing: it must match `LINK_COLUMNS`, which supplies the
 * `<colgroup>` widths and the headers. Every cell here therefore truncates rather
 * than expands — the column decides the width, never the content.
 */
export const UrlTableRow: React.FC<UrlTableRowProps> = ({
    url,
    shortLink,
    isAdmin,
    controller,
    index,
    measureRef,
}) => {
    const detailsPath = `/url/${encodeShortCodeForApi(url.short_code)}`;
    const { spec, isRunning, disabled } = rowActionState(url, controller);
    const ActionIcon = spec.icon;

    return (
        <tr
            data-index={index}
            ref={measureRef}
            className={cn('group border-b border-border/60 transition-colors', ROW_HOVER)}
        >
            <TD className={stickyBodyCell('left')}>
                <Link
                    to={detailsPath}
                    title={url.short_code}
                    className="block truncate font-mono text-sm font-semibold text-primary hover:underline"
                >
                    {url.short_code}
                </Link>
            </TD>

            <TD>
                <a
                    href={url.original_url}
                    target="_blank"
                    rel="noopener noreferrer"
                    title={url.original_url}
                    className="flex items-center gap-1.5 text-fg-muted hover:text-fg hover:underline"
                >
                    <span className="truncate">{url.original_url}</span>
                    <ExternalLink className="h-3.5 w-3.5 shrink-0 opacity-60" />
                </a>
            </TD>

            <TD className="text-right font-medium tabular-nums">{url.clicks.toLocaleString()}</TD>

            <TD>
                <Badge tone={url.is_active ? 'success' : 'danger'} dot>
                    {url.is_active ? 'Active' : 'Inactive'}
                </Badge>
            </TD>

            {/* Compact in the cell, full precision in the tooltip. `tabular-nums` so
                the all-numeric timestamps line up column-wise down the table. */}
            <TD
                className="truncate tabular-nums text-fg-muted"
                title={formatDate(url.created_at)}
            >
                {formatDateCompact(url.created_at)}
            </TD>

            {isAdmin && (
                <TD
                    className="truncate font-mono text-xs text-fg-muted"
                    title={url.created_by ?? undefined}
                >
                    {url.created_by ? shortenPrincipal(url.created_by) : ANONYMOUS_PRINCIPAL}
                </TD>
            )}

            <TD className={stickyBodyCell('right')}>
                <div className="flex items-center justify-end gap-1">
                    {shortLink && (
                        <CopyButton
                            value={shortLink}
                            iconOnly
                            variant="ghost"
                            size="sm"
                            idleLabel="Copy link"
                            copiedLabel="Link copied"
                            // Square like its neighbours: `Button`'s size padding is
                            // responsive, so both breakpoints must be cleared.
                            className="h-8 w-8 px-0 sm:px-0"
                        />
                    )}
                    <Link
                        to={detailsPath}
                        aria-label="View analytics"
                        title="View analytics"
                        className={iconButtonClass({ size: 'sm' })}
                    >
                        <BarChart3 className="h-4 w-4" />
                    </Link>
                    {isAdmin && (
                        <IconButton
                            size="sm"
                            variant={spec.tone}
                            label={spec.label}
                            isLoading={isRunning}
                            disabled={disabled}
                            onClick={() => controller.request(url)}
                        >
                            <ActionIcon className="h-4 w-4" />
                        </IconButton>
                    )}
                </div>
            </TD>
        </tr>
    );
};
