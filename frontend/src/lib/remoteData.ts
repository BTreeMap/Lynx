/**
 * The state of one remote read, as a closed union.
 *
 * A request that has not been made, one in flight, one that produced a value, and one
 * that failed are four mutually exclusive states. Spread across independent
 * `isLoading` / `data` / `error` fields they form eight combinations, of which four
 * are unreachable — including "loading with a stale value and an error", which is what
 * a component ends up rendering when a second request settles out of order.
 *
 * The union is erased at runtime; only the tag string survives.
 */
export type RemoteData<T> =
    | { readonly tag: 'idle' }
    | { readonly tag: 'loading' }
    | { readonly tag: 'success'; readonly value: T }
    | { readonly tag: 'failure'; readonly message: string };

/** Shared singletons: the payload-free variants carry no identity worth allocating. */
export const IDLE: RemoteData<never> = { tag: 'idle' };
export const LOADING: RemoteData<never> = { tag: 'loading' };

export const success = <T>(value: T): RemoteData<T> => ({ tag: 'success', value });

export const failure = <T>(message: string): RemoteData<T> => ({ tag: 'failure', message });

/**
 * Total eliminator. Every variant must be handled, so adding one is a compile error at
 * each call site rather than a silently empty render.
 */
export const foldRemote = <T, R>(
    state: RemoteData<T>,
    handlers: {
        readonly onIdle: () => R;
        readonly onLoading: () => R;
        readonly onSuccess: (value: T) => R;
        readonly onFailure: (message: string) => R;
    },
): R => {
    switch (state.tag) {
        case 'idle':
            return handlers.onIdle();
        case 'loading':
            return handlers.onLoading();
        case 'success':
            return handlers.onSuccess(state.value);
        case 'failure':
            return handlers.onFailure(state.message);
    }
};

/** Projection to `Option`, for derivations that treat every non-success alike. */
export const valueOf = <T>(state: RemoteData<T>): T | undefined =>
    state.tag === 'success' ? state.value : undefined;

/**
 * Substitute a fallback state for an expected failure. Used where a section is
 * *designed* to degrade — analytics the backend may not be collecting at all — so that
 * "the endpoint is unavailable" and "there is nothing to show yet" render identically.
 *
 * The fallback is a value, not a thunk, so callers hoist one shared instance to module
 * scope: recovering with a freshly built state on every render would defeat the
 * downstream `useMemo`s that key on this object's identity.
 */
export const recover = <T>(state: RemoteData<T>, fallback: RemoteData<T>): RemoteData<T> =>
    state.tag === 'failure' ? fallback : state;
