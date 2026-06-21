import React, { useState } from 'react';
import { Link } from 'react-router-dom';
import { BarChart3, ExternalLink, PowerOff, RotateCcw } from 'lucide-react';
import { apiClient } from '../api';
import type { ShortenedUrl } from '../types';
import { buildShortLink, encodeShortCodeForApi } from '../utils/url';
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

const formatDate = (timestamp: number) => new Date(timestamp * 1000).toLocaleString();

const UrlList: React.FC<UrlListProps> = ({ urls, isAdmin, onUrlsChanged }) => {
    const [actionInProgress, setActionInProgress] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [pending, setPending] = useState<PendingAction>(null);

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
            const apiError = err as { response?: { data?: { error?: string } } };
            setError(apiError.response?.data?.error || `Failed to ${type} URL`);
        } finally {
            setActionInProgress(null);
        }
    };

    const linkFor = (item: ShortenedUrl) => buildShortLink(item.short_code, item.redirect_base_url);

    return (
        <div className="space-y-4">
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
                        {urls.map((url) => {
                            const link = linkFor(url);
                            const busy = actionInProgress === url.short_code;
                            return (
                                <TR key={url.id} className="transition-colors hover:bg-surface-2/50">
                                    <TD>
                                        <Link
                                            to={`/url/${encodeShortCodeForApi(url.short_code)}`}
                                            className="inline-flex items-center gap-1.5 font-mono text-sm font-semibold text-primary hover:underline"
                                        >
                                            {url.short_code}
                                        </Link>
                                    </TD>
                                    <TD className="max-w-[22rem]">
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
                                        <div className="flex items-center justify-end gap-2">
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
                                </TR>
                            );
                        })}
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
