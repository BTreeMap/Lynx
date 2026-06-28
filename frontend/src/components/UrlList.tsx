import React, { useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { useWindowVirtualizer } from '@tanstack/react-virtual';
import { BarChart3, ExternalLink, PowerOff, RotateCcw } from 'lucide-react';
import { apiClient } from '../api';
import type { ShortenedUrl } from '../types';
import { buildShortLink, encodeShortCodeForApi } from '../utils/url';
import { formatDate } from '../utils/date';
import { extractErrorMessage } from '../utils/errorHandling';
import { Badge } from './ui/Badge';
import { Button } from './ui/Button';
import { CopyButton } from './ui/CopyButton';
import { Alert } from './ui/Alert';
import { Dialog } from './ui/Dialog';
import { Table, TBody, TD, TH, THead, TR, TableScroll } from './ui/Table';

interface UrlListProps {
    urls: ShortenedUrl[];
    isAdmin: boolean;
    onUrlsChanged: () => void;
}

type PendingAction = { code: string; type: 'deactivate' | 'reactivate' } | null;

const UrlList: React.FC<UrlListProps> = ({ urls, isAdmin, onUrlsChanged }) => {
    const [actionInProgress, setActionInProgress] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [pending, setPending] = useState<PendingAction>(null);
    const listRef = useRef<HTMLDivElement>(null);

    // Only the rows in (or near) the viewport are mounted in the DOM. The full
    // dataset stays in `urls` (cheap JS objects); virtualizing against the page
    // scroll keeps the DOM small even after "Load all" pulls in thousands of rows.
    const rowVirtualizer = useWindowVirtualizer({
        count: urls.length,
        estimateSize: () => 57,
        overscan: 12,
        scrollMargin: listRef.current?.offsetTop ?? 0,
    });
    const virtualItems = rowVirtualizer.getVirtualItems();
    const totalSize = rowVirtualizer.getTotalSize();
    const scrollMargin = rowVirtualizer.options.scrollMargin;
    const paddingTop = virtualItems.length ? virtualItems[0].start - scrollMargin : 0;
    const paddingBottom = virtualItems.length
        ? totalSize - (virtualItems[virtualItems.length - 1].end - scrollMargin)
        : 0;
    const colCount = isAdmin ? 7 : 6;

    const runAction = async () => {
        if (!pending) return;
        const { code, type } = pending;
        setPending(null);
        setActionInProgress(code);
        setError(null);
        try {
            if (type === 'deactivate') {
                await apiClient.deactivateUrl(code);
            } else {
                await apiClient.reactivateUrl(code);
            }
            onUrlsChanged();
        } catch (err: unknown) {
            setError(extractErrorMessage(err, `Failed to ${type} URL`));
        } finally {
            setActionInProgress(null);
        }
    };

    const linkFor = (item: ShortenedUrl) => buildShortLink(item.short_code, item.redirect_base_url);

    return (
        <div ref={listRef} className="space-y-3 sm:space-y-4">
            {error && <Alert tone="error">{error}</Alert>}

            <TableScroll>
                <Table>
                    <THead>
                        <TR className="border-b-0">
                            <TH>Short code</TH>
                            <TH>Destination</TH>
                            <TH className="text-right">Clicks</TH>
                            <TH>Status</TH>
                            <TH>Created</TH>
                            {isAdmin && <TH>Created by</TH>}
                            <TH className="text-right">Actions</TH>
                        </TR>
                    </THead>
                    <TBody>
                        {paddingTop > 0 && (
                            <tr aria-hidden>
                                <td colSpan={colCount} style={{ height: paddingTop }} />
                            </tr>
                        )}
                        {virtualItems.map((virtualRow) => {
                            const url = urls[virtualRow.index];
                            const link = linkFor(url);
                            const busy = actionInProgress === url.short_code;
                            return (
                                <tr
                                    key={url.id}
                                    data-index={virtualRow.index}
                                    ref={rowVirtualizer.measureElement}
                                    className="border-b border-border/60 transition-colors hover:bg-surface-2/50"
                                >
                                    <TD>
                                        <Link
                                            to={`/url/${encodeShortCodeForApi(url.short_code)}`}
                                            className="inline-flex items-center gap-1.5 font-mono text-sm font-semibold text-primary hover:underline"
                                        >
                                            {url.short_code}
                                        </Link>
                                    </TD>
                                    <TD className="max-w-72 sm:max-w-96">
                                        <a
                                            href={url.original_url}
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            title={url.original_url}
                                            className="inline-flex max-w-full items-center gap-1.5 truncate text-fg-muted hover:text-fg hover:underline"
                                        >
                                            <span className="truncate">{url.original_url}</span>
                                            <ExternalLink className="h-3.5 w-3.5 shrink-0 opacity-60" />
                                        </a>
                                    </TD>
                                    <TD className="text-right font-medium tabular-nums">
                                        {url.clicks.toLocaleString()}
                                    </TD>
                                    <TD>
                                        <Badge tone={url.is_active ? 'success' : 'danger'} dot>
                                            {url.is_active ? 'Active' : 'Inactive'}
                                        </Badge>
                                    </TD>
                                    <TD className="whitespace-nowrap text-fg-muted">{formatDate(url.created_at)}</TD>
                                    {isAdmin && (
                                        <TD className="whitespace-nowrap text-fg-muted">{url.created_by || '—'}</TD>
                                    )}
                                    <TD>
                                        <div className="flex items-center justify-end gap-1.5 sm:gap-2">
                                            {link && (
                                                <CopyButton
                                                    value={link}
                                                    iconOnly
                                                    variant="ghost"
                                                    size="sm"
                                                    idleLabel="Copy link"
                                                    copiedLabel="Copied"
                                                    className="px-2"
                                                />
                                            )}
                                            <Link
                                                to={`/url/${encodeShortCodeForApi(url.short_code)}`}
                                                aria-label="View analytics"
                                                title="View analytics"
                                                className="inline-flex h-8 w-8 items-center justify-center rounded-lg text-fg-muted transition-colors hover:bg-surface-2 hover:text-fg focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring"
                                            >
                                                <BarChart3 className="h-4 w-4" />
                                            </Link>
                                            {isAdmin &&
                                                (url.is_active ? (
                                                    <Button
                                                        variant="danger"
                                                        size="sm"
                                                        isLoading={busy}
                                                        onClick={() => setPending({ code: url.short_code, type: 'deactivate' })}
                                                        leftIcon={!busy ? <PowerOff className="h-4 w-4" /> : undefined}
                                                    >
                                                        Deactivate
                                                    </Button>
                                                ) : (
                                                    <Button
                                                        variant="success"
                                                        size="sm"
                                                        isLoading={busy}
                                                        onClick={() => setPending({ code: url.short_code, type: 'reactivate' })}
                                                        leftIcon={!busy ? <RotateCcw className="h-4 w-4" /> : undefined}
                                                    >
                                                        Reactivate
                                                    </Button>
                                                ))}
                                        </div>
                                    </TD>
                                </tr>
                            );
                        })}
                        {paddingBottom > 0 && (
                            <tr aria-hidden>
                                <td colSpan={colCount} style={{ height: paddingBottom }} />
                            </tr>
                        )}
                    </TBody>
                </Table>
            </TableScroll>

            <Dialog
                open={pending !== null}
                onClose={() => setPending(null)}
                title={pending?.type === 'deactivate' ? 'Deactivate link?' : 'Reactivate link?'}
                description={
                    pending?.type === 'deactivate'
                        ? 'Visitors will no longer be redirected. You can reactivate it later.'
                        : 'The link will start redirecting visitors again.'
                }
                footer={
                    <>
                        <Button variant="secondary" onClick={() => setPending(null)}>
                            Cancel
                        </Button>
                        <Button
                            variant={pending?.type === 'deactivate' ? 'danger' : 'success'}
                            onClick={runAction}
                        >
                            {pending?.type === 'deactivate' ? 'Deactivate' : 'Reactivate'}
                        </Button>
                    </>
                }
            >
                <p className="rounded-lg border border-border bg-surface-2/60 px-3 py-2 font-mono text-sm text-fg">
                    {pending?.code}
                </p>
            </Dialog>
        </div>
    );
};

export default UrlList;
