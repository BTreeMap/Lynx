import { useCallback, useReducer } from 'react';
import { PowerOff, RotateCcw, type LucideIcon } from 'lucide-react';
import { apiClient } from '../../api';
import { assertNever } from '../../lib/assertNever';
import type { ShortenedUrl } from '../../types';
import { extractErrorMessage } from '../../utils/errorHandling';

/* ---------------------------------------------------------------------------
   The action, as a closed variant set
--------------------------------------------------------------------------- */

export type UrlActionKind = 'deactivate' | 'reactivate';

/** Everything any view needs in order to offer, confirm, and perform an action. */
export interface UrlActionSpec {
    readonly kind: UrlActionKind;
    /** Doubles as the tooltip and accessible name of the icon-only control. */
    readonly label: string;
    readonly icon: LucideIcon;
    /** Valid for both `Button` and `IconButton`, so one field drives every surface. */
    readonly tone: 'danger' | 'success';
    readonly confirmTitle: string;
    readonly confirmBody: string;
    readonly failureMessage: string;
    readonly run: (code: string) => Promise<unknown>;
}

/**
 * A `Record` over the closed `UrlActionKind` union is the elimination table: it is
 * total by construction, so a third action cannot be introduced without supplying
 * every field its callers read. This replaces four `type === 'deactivate' ? … : …`
 * ternaries that were spread across the dialog and both row renderers — each of
 * which was an independent opportunity for the copy, the tone, and the request to
 * disagree about which action the user had actually chosen.
 */
export const URL_ACTIONS: Readonly<Record<UrlActionKind, UrlActionSpec>> = {
    deactivate: {
        kind: 'deactivate',
        label: 'Deactivate',
        icon: PowerOff,
        tone: 'danger',
        confirmTitle: 'Deactivate link?',
        confirmBody: 'Visitors will no longer be redirected. You can reactivate it later.',
        failureMessage: 'Failed to deactivate URL',
        run: (code) => apiClient.deactivateUrl(code),
    },
    reactivate: {
        kind: 'reactivate',
        label: 'Reactivate',
        icon: RotateCcw,
        tone: 'success',
        confirmTitle: 'Reactivate link?',
        confirmBody: 'The link will start redirecting visitors again.',
        failureMessage: 'Failed to reactivate URL',
        run: (code) => apiClient.reactivateUrl(code),
    },
};

/** The one place a link's `is_active` flag is turned into an offered action. */
export const actionFor = (url: ShortenedUrl): UrlActionSpec =>
    url.is_active ? URL_ACTIONS.deactivate : URL_ACTIONS.reactivate;

/* ---------------------------------------------------------------------------
   The transition machine
--------------------------------------------------------------------------- */

/**
 * Previously three independent nullable fields (`pending`, `actionInProgress`,
 * `error`) whose eight combinations included states the flow can never be in —
 * notably "confirming one action while another runs", which shares a single spinner
 * slot and so attributes progress to the wrong row. As one union, confirming and
 * running are mutually exclusive by type, and the confirmed action travels *with*
 * the phase instead of being re-derived from a nullable while the dialog closes.
 */
export type ActionPhase =
    | { readonly tag: 'idle' }
    | { readonly tag: 'confirming'; readonly code: string; readonly spec: UrlActionSpec }
    | { readonly tag: 'running'; readonly code: string; readonly spec: UrlActionSpec };

/**
 * The error is a product with the phase, not another variant of it: a failed attempt
 * stays on screen while the user opens the next confirmation, which is the existing
 * behaviour and the useful one.
 */
interface ActionState {
    readonly phase: ActionPhase;
    readonly error: string | null;
}

type ActionEvent =
    | { readonly type: 'requested'; readonly code: string; readonly spec: UrlActionSpec }
    | { readonly type: 'cancelled' }
    | { readonly type: 'started' }
    | { readonly type: 'succeeded' }
    | { readonly type: 'failed'; readonly message: string };

const IDLE: ActionState = { phase: { tag: 'idle' }, error: null };

/**
 * Total over every state x event pair, and framework-free so it can be tested
 * directly. Events that cannot apply to the current phase return the state
 * unchanged: mutations are sequential by construction, and a settlement arriving
 * outside `running` is stale and must not resurrect a finished action.
 */
export const urlActionReducer = (state: ActionState, event: ActionEvent): ActionState => {
    switch (event.type) {
        case 'requested':
            return state.phase.tag === 'idle'
                ? { ...state, phase: { tag: 'confirming', code: event.code, spec: event.spec } }
                : state;
        case 'cancelled':
            return state.phase.tag === 'confirming' ? { ...state, phase: { tag: 'idle' } } : state;
        case 'started':
            return state.phase.tag === 'confirming'
                ? {
                      phase: { tag: 'running', code: state.phase.code, spec: state.phase.spec },
                      error: null,
                  }
                : state;
        case 'succeeded':
            return state.phase.tag === 'running' ? IDLE : state;
        case 'failed':
            return state.phase.tag === 'running'
                ? { phase: { tag: 'idle' }, error: event.message }
                : state;
        default:
            return assertNever(event);
    }
};

/* ---------------------------------------------------------------------------
   The effect boundary
--------------------------------------------------------------------------- */

export interface UrlActionsController {
    readonly phase: ActionPhase;
    readonly error: string | null;
    /** Opens the confirmation for whichever action this link currently offers. */
    readonly request: (url: ShortenedUrl) => void;
    readonly confirm: () => void;
    readonly cancel: () => void;
    /** Short code of the in-flight mutation, if any — derived, not tracked. */
    readonly runningCode: string | null;
    /** True while a mutation is in flight; other rows' toggles disable rather than
     *  accept a click the sequential machine would silently drop. */
    readonly isBusy: boolean;
}

/**
 * Interprets the machine above against the API. This hook is the only impure part of
 * the action model: the reducer decides *what* may happen, this decides *when* the
 * request goes out. The request is fired from the confirm handler rather than an
 * effect keyed on state, because it is caused by a user event and nothing else.
 */
export const useUrlActions = (onUrlsChanged: () => void): UrlActionsController => {
    const [state, dispatch] = useReducer(urlActionReducer, IDLE);
    const { phase, error } = state;

    const request = useCallback((url: ShortenedUrl) => {
        dispatch({ type: 'requested', code: url.short_code, spec: actionFor(url) });
    }, []);

    const cancel = useCallback(() => dispatch({ type: 'cancelled' }), []);

    const confirm = useCallback(() => {
        if (phase.tag !== 'confirming') return;
        const { code, spec } = phase;
        dispatch({ type: 'started' });
        // Not awaited by the caller: the machine, not the call site, owns what the UI
        // shows while this is in flight. `apiClient` exposes no `AbortSignal`, so an
        // unmount mid-flight lets the request complete and the dispatches no-op.
        void spec
            .run(code)
            .then(
                () => {
                    dispatch({ type: 'succeeded' });
                    onUrlsChanged();
                },
                (err: unknown) => {
                    dispatch({
                        type: 'failed',
                        message: extractErrorMessage(err, spec.failureMessage),
                    });
                },
            );
    }, [phase, onUrlsChanged]);

    return {
        phase,
        error,
        request,
        confirm,
        cancel,
        runningCode: phase.tag === 'running' ? phase.code : null,
        isBusy: phase.tag === 'running',
    };
};

/* ---------------------------------------------------------------------------
   Per-row derivation shared by every presentation
--------------------------------------------------------------------------- */

export interface RowActionState {
    readonly spec: UrlActionSpec;
    readonly isRunning: boolean;
    readonly disabled: boolean;
}

/**
 * Derived during render instead of stored: which action a row offers, and whether it
 * is the one in flight, are functions of the link and the controller. The card and
 * the table row render this differently but must never disagree about it.
 */
export const rowActionState = (
    url: ShortenedUrl,
    controller: UrlActionsController,
): RowActionState => {
    const isRunning = controller.runningCode === url.short_code;
    return { spec: actionFor(url), isRunning, disabled: controller.isBusy && !isRunning };
};
