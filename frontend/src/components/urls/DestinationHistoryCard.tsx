import React from 'react';
import { History, RotateCcw } from 'lucide-react';
import { foldRemote, type RemoteData } from '../../lib/remoteData';
import type { UrlHistoryEntry } from '../../types';
import { formatDate } from '../../utils/date';
import { ANONYMOUS_PRINCIPAL } from '../../utils/identity';
import { Alert } from '../ui/Alert';
import { Button } from '../ui/Button';
import { Card, CardBody, CardSectionHeader, CardTitle } from '../ui/Card';
import { EmptyState } from '../ui/EmptyState';
import { Skeleton } from '../ui/Skeleton';
import { Table, TBody, TD, TH, THead, TR, TableScroll } from '../ui/Table';

export interface DestinationHistoryCardProps {
    readonly history: RemoteData<readonly UrlHistoryEntry[]>;
    readonly onRestore: (entry: UrlHistoryEntry) => void;
}

const emptyState = (
    <EmptyState
        icon={<History className="h-6 w-6" />}
        title="No previous destinations"
        description="When you change this link's destination, the previous values appear here so you can restore them."
    />
);

/**
 * Every destination this link has had.
 *
 * The four render states are the four variants of the query, matched exhaustively —
 * where the page previously threaded `isLoadingHistory` and `historyError` through a
 * chain of ternaries whose last branch stood for both "loaded and empty" and "never
 * asked".
 */
export const DestinationHistoryCard: React.FC<DestinationHistoryCardProps> = ({
    history,
    onRestore,
}) => (
    <Card>
        <CardSectionHeader>
            <CardTitle>Destination history</CardTitle>
        </CardSectionHeader>
        <CardBody>
            {foldRemote(history, {
                onIdle: () => emptyState,
                onLoading: () => (
                    <div className="space-y-2.5 sm:space-y-3">
                        <Skeleton className="h-12 w-full" />
                        <Skeleton className="h-12 w-full" />
                    </div>
                ),
                onFailure: (message) => <Alert tone="error">{message}</Alert>,
                onSuccess: (entries) =>
                    entries.length === 0 ? (
                        emptyState
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
                                    {entries.map((entry) => (
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
                                            <TD className="text-fg-muted">
                                                {entry.changed_by || ANONYMOUS_PRINCIPAL}
                                            </TD>
                                            <TD className="text-right">
                                                <Button
                                                    variant="secondary"
                                                    size="sm"
                                                    onClick={() => onRestore(entry)}
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
                    ),
            })}
        </CardBody>
    </Card>
);
