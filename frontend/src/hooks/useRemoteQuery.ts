import { useCallback, useEffect, useState } from 'react';
import { failure, IDLE, LOADING, success, type RemoteData } from '../lib/remoteData';
import { extractErrorMessage } from '../utils/errorHandling';

/**
 * A read, expressed as a function of a cancellation signal. Callers stabilise it with
 * `useMemo`/`useCallback` so its identity *is* the query's key: a new fetcher means a
 * new query, and an unchanged one means no work.
 */
export type Fetcher<T> = (signal: AbortSignal) => Promise<T>;

export interface RemoteQuery<T> {
    readonly state: RemoteData<T>;
    /** Re-run the current fetcher (after a mutation, or a user-initiated retry). */
    readonly reload: () => void;
    /**
     * Adopt a value obtained elsewhere — a mutation response that *is* the new state of
     * the resource. Skips the round trip a `reload` would spend re-reading what the
     * server just returned.
     */
    readonly replace: (value: T) => void;
}

/** A settlement, tagged with the query it answers. */
interface Settled<T> {
    readonly fetcher: Fetcher<T> | null;
    readonly epoch: number;
    readonly data: RemoteData<T>;
}

/**
 * The app's single effect boundary for reading.
 *
 * Six copies of this lifecycle existed inline across the details route and the
 * dashboard, each with its own `isLoadingX` flag, its own `try`/`catch`/`finally`, and
 * none with cancellation: switching the analytics dimension twice in quick succession
 * left whichever response happened to land last on screen, regardless of which
 * dimension was actually selected.
 *
 * Two properties make that impossible here:
 *
 *  - the request is aborted when its key changes or the component unmounts, and a
 *    settlement is admitted only while its own signal is live;
 *  - `idle` and `loading` are *derived* during render from the key, never written. Only
 *    settlements are stored, tagged with the query that produced them, so a result whose
 *    key has moved on is simply not the current state — there is no window in which a
 *    stale value is displayed and no cascading render to correct it.
 *
 * A `null` fetcher means "no query to run" (a route parameter that failed to parse, a
 * resource not yet identified) and yields `idle` — an explicit fourth state rather than
 * a `loading` that will never settle.
 */
export const useRemoteQuery = <T>(
    fetcher: Fetcher<T> | null,
    fallbackMessage: string,
): RemoteQuery<T> => {
    const [settled, setSettled] = useState<Settled<T> | null>(null);
    const [epoch, setEpoch] = useState(0);

    useEffect(() => {
        if (fetcher === null) return;

        const controller = new AbortController();
        fetcher(controller.signal).then(
            (value) => {
                if (!controller.signal.aborted) {
                    setSettled({ fetcher, epoch, data: success(value) });
                }
            },
            (error: unknown) => {
                // An abort rejects too; it is this component's own cleanup, not a failure
                // worth showing anyone.
                if (!controller.signal.aborted) {
                    setSettled({
                        fetcher,
                        epoch,
                        data: failure(extractErrorMessage(error, fallbackMessage)),
                    });
                }
            },
        );

        return () => controller.abort();
    }, [fetcher, fallbackMessage, epoch]);

    const isCurrent = settled !== null && settled.fetcher === fetcher && settled.epoch === epoch;
    const state: RemoteData<T> = isCurrent ? settled.data : fetcher === null ? IDLE : LOADING;

    const reload = useCallback(() => setEpoch((previous) => previous + 1), []);
    const replace = useCallback(
        (value: T) => setSettled({ fetcher, epoch, data: success(value) }),
        [fetcher, epoch],
    );

    return { state, reload, replace };
};
