import { assertNever } from '../lib/assertNever';
import type { AuthModeResponse, OAuthFrontendConfig, UserInfo } from '../types';

/* ---------------------------------------------------------------------------
   Server-declared configuration — parsed, not merely received
--------------------------------------------------------------------------- */

/** How the server establishes the caller's identity. Closed set; see `src/auth/`. */
export type AuthMode = 'none' | 'cloudflare' | 'oauth';

/**
 * OAuth client configuration exists in exactly one mode, so it is a field of that
 * variant rather than a nullable field of every configuration. Previously the pair
 * `(mode, oauthConfig)` admitted "cloudflare with an OAuth client", which no branch
 * could act on but every branch had to consider.
 *
 * `oauth: null` inside the `oauth` variant is a *reachable* state — an instance
 * advertising OAuth without publishing a client — and the sign-in path reports it.
 */
export type AuthConfig =
    | { readonly mode: 'none' }
    | { readonly mode: 'cloudflare' }
    | { readonly mode: 'oauth'; readonly oauth: OAuthFrontendConfig | null };

export interface ServerConfig {
    readonly auth: AuthConfig;
    readonly shortCodeMaxLength: number;
}

export const DEFAULT_SHORT_CODE_MAX_LENGTH = 50;

/**
 * Used when `/auth/mode` cannot be read at all. OAuth is the safe default: it is the
 * only mode that withholds the dashboard until a credential exists, so a failed probe
 * cannot open an instance that is actually protected.
 */
export const FALLBACK_SERVER_CONFIG: ServerConfig = {
    auth: { mode: 'oauth', oauth: null },
    shortCodeMaxLength: DEFAULT_SHORT_CODE_MAX_LENGTH,
};

const parseAuthConfig = (raw: AuthModeResponse): AuthConfig => {
    switch (raw.mode) {
        case 'none':
            return { mode: 'none' };
        case 'cloudflare':
            return { mode: 'cloudflare' };
        case 'oauth':
            return { mode: 'oauth', oauth: raw.oauth ?? null };
        default:
            // An unrecognised mode is a server the client does not understand. Treat it
            // as the restrictive mode rather than admitting the string into the domain,
            // where every `mode === …` comparison would silently answer "no".
            return { mode: 'oauth', oauth: raw.oauth ?? null };
    }
};

/**
 * The single boundary where the untrusted `/auth/mode` payload becomes a domain value.
 * The response is JSON: its `mode` is `string` and its length is `number`, and neither
 * is constrained by the type declarations the compiler erases.
 */
export const parseServerConfig = (raw: AuthModeResponse): ServerConfig => ({
    auth: parseAuthConfig(raw),
    shortCodeMaxLength:
        Number.isInteger(raw.short_code_max_length) && raw.short_code_max_length > 0
            ? raw.short_code_max_length
            : DEFAULT_SHORT_CODE_MAX_LENGTH,
});

/* ---------------------------------------------------------------------------
   Session state
--------------------------------------------------------------------------- */

/**
 * What the client presents to the API.
 *
 * `ambient` covers `none` and `cloudflare`, where identity is established by the
 * transport (a proxy header, or nothing at all) and the client holds no secret. That
 * is a different thing from holding a token, and the two used to be conflated by
 * testing `authMode === 'none' || authMode === 'cloudflare' || token` at four call
 * sites — a disjunction that had to be re-derived correctly every time.
 */
export type Credential =
    | { readonly tag: 'bearer'; readonly token: string }
    | { readonly tag: 'ambient' };

export type Session =
    /** OAuth mode with no token: the dashboard is withheld and sign-in is offered. */
    | { readonly tag: 'anonymous' }
    /** A credential exists; the identity it names is still being read. */
    | { readonly tag: 'identifying'; readonly credential: Credential }
    /** Authorized. `user === null` means the identity probe failed but the credential
     *  stands — the API, not the client, is the authority on what it permits. */
    | { readonly tag: 'authorized'; readonly credential: Credential; readonly user: UserInfo | null };

export type AuthState =
    | { readonly tag: 'initializing' }
    | { readonly tag: 'ready'; readonly config: ServerConfig; readonly session: Session };

export const INITIALIZING: AuthState = { tag: 'initializing' };

export type AuthEvent =
    /** `/auth/mode` settled (with the fallback config if it failed). */
    | { readonly type: 'configured'; readonly config: ServerConfig; readonly token: string | null }
    | { readonly type: 'identified'; readonly user: UserInfo | null }
    | { readonly type: 'signedIn'; readonly token: string }
    | { readonly type: 'signedOut' };

/**
 * The initial session is a function of the parsed configuration and whatever token
 * survived the last visit — the one place the pass-through modes are turned into a
 * credential.
 */
const openSession = (config: AuthConfig, token: string | null): Session => {
    switch (config.mode) {
        case 'none':
        case 'cloudflare':
            return { tag: 'identifying', credential: { tag: 'ambient' } };
        case 'oauth':
            return token === null
                ? { tag: 'anonymous' }
                : { tag: 'identifying', credential: { tag: 'bearer', token } };
        default:
            return assertNever(config);
    }
};

/**
 * Total over every state × event pair, and free of React so it can be exercised
 * directly. Events that cannot apply to the current state return it unchanged: an
 * identity arriving after sign-out is stale and must not resurrect a closed session.
 */
export const authReducer = (state: AuthState, event: AuthEvent): AuthState => {
    switch (event.type) {
        case 'configured':
            return state.tag === 'initializing'
                ? {
                      tag: 'ready',
                      config: event.config,
                      session: openSession(event.config.auth, event.token),
                  }
                : state;
        case 'identified':
            return state.tag === 'ready' && state.session.tag === 'identifying'
                ? {
                      ...state,
                      session: {
                          tag: 'authorized',
                          credential: state.session.credential,
                          user: event.user,
                      },
                  }
                : state;
        case 'signedIn':
            return state.tag === 'ready'
                ? {
                      ...state,
                      session: {
                          tag: 'identifying',
                          credential: { tag: 'bearer', token: event.token },
                      },
                  }
                : state;
        case 'signedOut':
            // Only a bearer credential can be surrendered: there is nothing to drop in
            // the pass-through modes, and dropping "nothing" would strand the app on a
            // sign-in screen its server has no flow for.
            return state.tag === 'ready' && state.config.auth.mode === 'oauth'
                ? { ...state, session: { tag: 'anonymous' } }
                : state;
        default:
            return assertNever(event);
    }
};

/* ---------------------------------------------------------------------------
   Selectors — derived during render, never stored
--------------------------------------------------------------------------- */

/** True while either the configuration or the identity is still unknown. */
export const isBooting = (state: AuthState): boolean =>
    state.tag === 'initializing' || state.session.tag === 'identifying';

/** True once the app may show the dashboard. */
export const isAuthorized = (state: AuthState): boolean =>
    state.tag === 'ready' && state.session.tag === 'authorized';

export const currentUser = (state: AuthState): UserInfo | null =>
    state.tag === 'ready' && state.session.tag === 'authorized' ? state.session.user : null;

export const isAdmin = (state: AuthState): boolean => currentUser(state)?.is_admin ?? false;

export const authMode = (state: AuthState): AuthMode | null =>
    state.tag === 'ready' ? state.config.auth.mode : null;

export const shortCodeMaxLength = (state: AuthState): number =>
    state.tag === 'ready' ? state.config.shortCodeMaxLength : DEFAULT_SHORT_CODE_MAX_LENGTH;

/**
 * The OAuth client, or `null` when this instance has none to offer. Reached only from
 * the `oauth` variant, so no other mode can accidentally start a flow.
 */
export const oauthClient = (state: AuthState): OAuthFrontendConfig | null =>
    state.tag === 'ready' && state.config.auth.mode === 'oauth' ? state.config.auth.oauth : null;

/** The credential whose identity is being read, if any — the honest effect key. */
export const pendingCredential = (state: AuthState): Credential | null =>
    state.tag === 'ready' && state.session.tag === 'identifying' ? state.session.credential : null;
