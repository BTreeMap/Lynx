import React, { useCallback, useMemo } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { apiClient } from '../api';
import { useRemoteQuery, type Fetcher } from '../hooks/useRemoteQuery';
import { valueOf } from '../lib/remoteData';
import type { ShortenedUrl, UrlHistoryEntry } from '../types';
import { decodeShortCodeFromApi } from '../utils/url';
import { DimensionAnalyticsCard } from './analytics/DimensionAnalyticsCard';
import { RecentActivityCard } from './analytics/RecentActivityCard';
import { AppHeader } from './layout/AppHeader';
import { PageIntro, PageShell } from './layout/Page';
import { Alert } from './ui/Alert';
import { Button } from './ui/Button';
import { DestinationDialogs } from './urls/DestinationDialogs';
import { DestinationHistoryCard } from './urls/DestinationHistoryCard';
import { LinkInfoCard } from './urls/LinkInfoCard';
import { useDestinationEditor } from './urls/destinationEditor';

/**
 * The link detail route: composition only.
 *
 * Each card owns the query it renders and each machine owns the state it advances, so
 * this file holds no `useState` at all. What is left is the route's own decisions — the
 * short code, and what to show when it does not parse.
 */
const UrlDetails: React.FC = () => {
    const { shortCode } = useParams<{ shortCode: string }>();
    const navigate = useNavigate();

    /*
      The one untrusted value on this route. It is parsed once, here, and every consumer
      below receives either a code that decoded or `null` — nothing re-runs the decode,
      and no request is issued for a code that could not be read.
    */
    const code = useMemo(
        () => (shortCode === undefined ? null : decodeShortCodeFromApi(shortCode)),
        [shortCode],
    );

    const urlFetcher = useMemo<Fetcher<ShortenedUrl> | null>(
        () => (code === null ? null : (signal) => apiClient.getUrl(code, { signal })),
        [code],
    );
    const historyFetcher = useMemo<Fetcher<readonly UrlHistoryEntry[]> | null>(
        () => (code === null ? null : (signal) => apiClient.getUrlHistory(code, { signal })),
        [code],
    );

    const urlQuery = useRemoteQuery(urlFetcher, 'Failed to load URL details');
    const historyQuery = useRemoteQuery(historyFetcher, 'Failed to load destination history');

    // A mutation returns the link's new state, so it is adopted directly; only the
    // history, which the server extends as a side effect, has to be re-read.
    const { replace: replaceUrl } = urlQuery;
    const { reload: reloadHistory } = historyQuery;
    const onUpdated = useCallback(
        (updated: ShortenedUrl) => {
            replaceUrl(updated);
            reloadHistory();
        },
        [replaceUrl, reloadHistory],
    );

    const editor = useDestinationEditor(code, onUpdated);

    const backToDashboard = (
        <Button
            variant="ghost"
            size="sm"
            onClick={() => navigate('/')}
            leftIcon={<ArrowLeft className="h-4 w-4" />}
            className="-ml-2 w-fit"
        >
            Back to dashboard
        </Button>
    );

    // An unreadable short code and a link that failed to load are the same dead end: the
    // page has no subject, so it offers the way back instead of empty cards.
    const deadEnd =
        code === null
            ? 'Invalid short code'
            : urlQuery.state.tag === 'failure'
              ? urlQuery.state.message
              : null;

    if (deadEnd !== null) {
        return (
            <div className="min-h-screen bg-bg">
                <AppHeader />
                <PageShell className="px-3 py-8 sm:px-6 sm:py-10">
                    <Alert tone="error">{deadEnd}</Alert>
                    <Button
                        variant="secondary"
                        className="mt-6"
                        onClick={() => navigate('/')}
                        leftIcon={<ArrowLeft className="h-4 w-4" />}
                    >
                        Back to dashboard
                    </Button>
                </PageShell>
            </div>
        );
    }

    return (
        <div className="min-h-screen bg-bg">
            <AppHeader />
            <PageShell className="overflow-x-clip">
                <PageIntro
                    title="Link analytics"
                    description="Detailed performance and audience insights for your short link."
                    actions={backToDashboard}
                />

                <LinkInfoCard url={valueOf(urlQuery.state) ?? null} onEdit={editor.openEdit} />
                <DestinationHistoryCard
                    history={historyQuery.state}
                    onRestore={editor.openRestore}
                />
                <DimensionAnalyticsCard code={code} />
                <RecentActivityCard code={code} />
            </PageShell>

            <DestinationDialogs editor={editor} />
        </div>
    );
};

export default UrlDetails;
