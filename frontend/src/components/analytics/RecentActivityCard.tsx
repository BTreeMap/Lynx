import React from 'react';
import { foldRemote } from '../../lib/remoteData';
import { formatTimeBucket } from '../../utils/date';
import { Card, CardBody, CardSectionHeader, CardTitle } from '../ui/Card';
import { EmptyState } from '../ui/EmptyState';
import { Skeleton } from '../ui/Skeleton';
import { Table, TBody, TD, TH, THead, TR, TableScroll } from '../ui/Table';
import { useRecentActivity } from './useAnalytics';

export interface RecentActivityCardProps {
    /** Decoded short code, or `null` when the route parameter did not parse. */
    readonly code: string | null;
}

/** The most recent visit buckets, newest first, exactly as the API returns them. */
export const RecentActivityCard: React.FC<RecentActivityCardProps> = ({ code }) => {
    const activity = useRecentActivity(code);

    const empty = (
        <EmptyState
            title="No recent activity"
            description="Analytics will appear once your link receives visits."
        />
    );

    return (
        <Card>
            <CardSectionHeader>
                <CardTitle>Recent activity</CardTitle>
            </CardSectionHeader>
            <CardBody>
                {foldRemote(activity, {
                    onIdle: () => empty,
                    onLoading: () => <Skeleton className="h-64 w-full" />,
                    // Recovered into an empty response upstream; kept for exhaustiveness.
                    onFailure: () => empty,
                    onSuccess: ({ entries }) =>
                        entries.length === 0 ? (
                            empty
                        ) : (
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
                                        {entries.map((entry) => (
                                            <TR key={entry.id}>
                                                <TD className="whitespace-nowrap">
                                                    {formatTimeBucket(entry.time_bucket)}
                                                </TD>
                                                <TD className="text-fg-muted">
                                                    {entry.country_code || 'N/A'}
                                                </TD>
                                                <TD className="text-fg-muted">
                                                    {entry.region || 'N/A'}
                                                </TD>
                                                <TD className="text-fg-muted">
                                                    {entry.city || 'N/A'}
                                                </TD>
                                                <TD className="text-right font-medium tabular-nums">
                                                    {entry.visit_count.toLocaleString()}
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
};
