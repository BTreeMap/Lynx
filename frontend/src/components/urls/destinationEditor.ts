import { useCallback, useReducer } from 'react';
import { apiClient } from '../../api';
import { assertNever } from '../../lib/assertNever';
import type { ShortenedUrl, UrlHistoryEntry } from '../../types';
import { extractErrorMessage } from '../../utils/errorHandling';

/**
 * Changing where a link points — by typing a new destination, or by restoring a
 * previous one.
 *
 * Both are the same operation on the same field, differing only in where the new value
 * comes from, and both were previously spelled out twice over six `useState` cells
 * (`isEditOpen`, `editValue`, `isSaving`, `editError`, `restoreTarget`, `isRestoring`,
 * `restoreError`). Those admit combinations the UI cannot mean — both dialogs open,
 * saving with the dialog shut, a restore error attached to no entry — and each dialog
 * had to re-derive its own copy of "am I busy".
 */

/** Whether a submission is in flight, and the failure left by the last one. */
export type SubmitStatus =
    | { readonly tag: 'idle'; readonly error: string | null }
    | { readonly tag: 'submitting' };

const READY: SubmitStatus = { tag: 'idle', error: null };

export type EditorState =
    | { readonly tag: 'closed' }
    | { readonly tag: 'editing'; readonly draft: string; readonly status: SubmitStatus }
    | {
          readonly tag: 'restoring';
          readonly entry: UrlHistoryEntry;
          readonly status: SubmitStatus;
      };

export const CLOSED: EditorState = { tag: 'closed' };

export type EditorEvent =
    | { readonly type: 'editOpened'; readonly current: string }
    | { readonly type: 'draftChanged'; readonly draft: string }
    | { readonly type: 'restoreOpened'; readonly entry: UrlHistoryEntry }
    | { readonly type: 'dismissed' }
    | { readonly type: 'submitted' }
    | { readonly type: 'succeeded' }
    | { readonly type: 'failed'; readonly message: string };

/** A submission is under way, so the dialog must not be dismissed from under it. */
const isSubmitting = (state: EditorState): boolean =>
    state.tag !== 'closed' && state.status.tag === 'submitting';

export const editorReducer = (state: EditorState, event: EditorEvent): EditorState => {
    switch (event.type) {
        case 'editOpened':
            return isSubmitting(state)
                ? state
                : { tag: 'editing', draft: event.current, status: READY };

        case 'draftChanged':
            return state.tag === 'editing' && state.status.tag === 'idle'
                ? { ...state, draft: event.draft }
                : state;

        case 'restoreOpened':
            return isSubmitting(state)
                ? state
                : { tag: 'restoring', entry: event.entry, status: READY };

        case 'dismissed':
            // Closing mid-flight would strand a request whose result has nowhere to land.
            return isSubmitting(state) ? state : CLOSED;

        case 'submitted':
            return state.tag === 'closed' || state.status.tag === 'submitting'
                ? state
                : { ...state, status: { tag: 'submitting' } };

        case 'succeeded':
            return isSubmitting(state) ? CLOSED : state;

        case 'failed':
            // Reachable from `idle` too: rejecting an empty destination is a failure the
            // client produces without ever leaving the dialog. A settlement arriving
            // after the dialog closed still finds `closed` and changes nothing.
            return state.tag === 'closed'
                ? state
                : { ...state, status: { tag: 'idle', error: event.message } };

        default:
            return assertNever(event);
    }
};

/* ---------------------------------------------------------------------------
   Selectors
--------------------------------------------------------------------------- */

export const editorError = (state: EditorState): string | null =>
    state.tag !== 'closed' && state.status.tag === 'idle' ? state.status.error : null;

export const editorBusy = isSubmitting;

/* ---------------------------------------------------------------------------
   The effect boundary
--------------------------------------------------------------------------- */

export interface DestinationEditor {
    readonly state: EditorState;
    readonly openEdit: (current: string) => void;
    readonly changeDraft: (draft: string) => void;
    readonly openRestore: (entry: UrlHistoryEntry) => void;
    readonly dismiss: () => void;
    /** Commit whichever change the current state describes. */
    readonly submit: () => void;
}

const EMPTY_DESTINATION = 'Destination cannot be empty';

/**
 * Interprets the machine against the API.
 *
 * The request is fired from `submit` — a user event — rather than from an effect keyed
 * on the state, so it happens once per click by construction and not once per render
 * that happens to observe `submitting`. `onUpdated` receives the server's new record,
 * which *is* the link's next state: the caller adopts it instead of re-reading it.
 */
export const useDestinationEditor = (
    code: string | null,
    onUpdated: (url: ShortenedUrl) => void,
): DestinationEditor => {
    const [state, dispatch] = useReducer(editorReducer, CLOSED);

    const openEdit = useCallback(
        (current: string) => dispatch({ type: 'editOpened', current }),
        [],
    );
    const changeDraft = useCallback(
        (draft: string) => dispatch({ type: 'draftChanged', draft }),
        [],
    );
    const openRestore = useCallback(
        (entry: UrlHistoryEntry) => dispatch({ type: 'restoreOpened', entry }),
        [],
    );
    const dismiss = useCallback(() => dispatch({ type: 'dismissed' }), []);

    const submit = useCallback(() => {
        if (code === null || state.tag === 'closed' || state.status.tag === 'submitting') {
            return;
        }

        // The two variants differ only here: what to send, and what to say when it fails.
        const attempt: { run: () => Promise<ShortenedUrl>; failure: string } | null =
            state.tag === 'editing'
                ? state.draft.trim().length === 0
                    ? null
                    : {
                          run: () => apiClient.updateUrl(code, state.draft.trim()),
                          failure: 'Failed to update destination',
                      }
                : {
                      run: () => apiClient.restoreUrl(code, state.entry.id),
                      failure: 'Failed to restore destination',
                  };

        if (attempt === null) {
            dispatch({ type: 'failed', message: EMPTY_DESTINATION });
            return;
        }

        dispatch({ type: 'submitted' });
        void attempt.run().then(
            (updated) => {
                dispatch({ type: 'succeeded' });
                onUpdated(updated);
            },
            (error: unknown) => {
                dispatch({ type: 'failed', message: extractErrorMessage(error, attempt.failure) });
            },
        );
    }, [code, state, onUpdated]);

    return { state, openEdit, changeDraft, openRestore, dismiss, submit };
};
