import { useCallback, useEffect, useRef, useState } from 'react';
import type { ShortenedUrl } from '../../types';
import { extractErrorMessage } from '../../utils/errorHandling';
import { ALL_LINKS } from './linkCollection';
import { fetchLinkPage } from './useLinkCollection';

/**
 * Export every link the caller can see as JSON.
 *
 * Deliberately independent of what the dashboard is currently showing: an export is of
 * the whole collection, not of the current search or of however far the user happened
 * to have paged. Pages are drained as fast as the server answers — unlike "Load all",
 * this is a single user-initiated download rather than a background fill, and there is
 * nothing on screen for it to pace against.
 */
const collectAllLinks = async (signal: AbortSignal): Promise<readonly ShortenedUrl[]> => {
    const pages: ShortenedUrl[] = [];
    let cursor: string | null = null;

    do {
        const page = await fetchLinkPage(ALL_LINKS, cursor, signal);
        pages.push(...page.items);
        cursor = page.nextCursor;
    } while (cursor !== null && !signal.aborted);

    return pages;
};

/** Hand a blob to the browser's download machinery and release it again. */
const downloadJson = (filename: string, payload: unknown): void => {
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
    const objectUrl = URL.createObjectURL(blob);
    try {
        const anchor = document.createElement('a');
        anchor.href = objectUrl;
        anchor.download = filename;
        // No need to attach it to the document: a detached anchor's `click()` starts the
        // download in every browser this app targets.
        anchor.click();
    } finally {
        // Paired with `createObjectURL` unconditionally — an exception between the two
        // would otherwise leak the blob for the lifetime of the document.
        URL.revokeObjectURL(objectUrl);
    }
};

export interface LinkExport {
    readonly isExporting: boolean;
    readonly error: string | null;
    readonly exportJson: () => void;
}

export const useLinkExport = (): LinkExport => {
    const [isExporting, setIsExporting] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const controllerRef = useRef<AbortController | null>(null);

    useEffect(() => () => controllerRef.current?.abort(), []);

    const exportJson = useCallback(() => {
        if (controllerRef.current !== null) return; // one export at a time
        const controller = new AbortController();
        controllerRef.current = controller;
        setIsExporting(true);
        setError(null);

        void collectAllLinks(controller.signal)
            .then((links) => {
                if (controller.signal.aborted) return;
                const today = new Date().toISOString().split('T')[0];
                downloadJson(`lynx-urls-export-${today}.json`, links);
            })
            .catch((cause: unknown) => {
                if (controller.signal.aborted) return;
                setError(extractErrorMessage(cause, 'Failed to export URLs'));
            })
            .finally(() => {
                controllerRef.current = null;
                if (!controller.signal.aborted) setIsExporting(false);
            });
    }, []);

    return { isExporting, error, exportJson };
};
