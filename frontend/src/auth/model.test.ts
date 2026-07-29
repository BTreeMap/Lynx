import { describe, expect, it } from 'vitest';
import type { AuthModeResponse, OAuthFrontendConfig } from '../types';
import {
    authReducer,
    currentUser,
    isAdmin,
    isAuthorized,
    isBooting,
    INITIALIZING,
    oauthClient,
    parseServerConfig,
    pendingCredential,
    shortCodeMaxLength,
    type AuthState,
} from './model';

const OAUTH_CLIENT: OAuthFrontendConfig = {
    issuer_url: 'https://idp.example',
    client_id: 'lynx',
    scopes: 'openid profile',
    redirect_uri: 'https://lynx.example/auth/callback',
};

const response = (overrides: Partial<AuthModeResponse> = {}): AuthModeResponse => ({
    mode: 'oauth',
    short_code_max_length: 50,
    ...overrides,
});

/** Boot the machine to `ready` with the given config and stored token. */
const boot = (raw: AuthModeResponse, token: string | null = null): AuthState =>
    authReducer(INITIALIZING, {
        type: 'configured',
        config: parseServerConfig(raw),
        token,
    });

describe('parseServerConfig', () => {
    it('admits the three known modes', () => {
        expect(parseServerConfig(response({ mode: 'none' })).auth).toEqual({ mode: 'none' });
        expect(parseServerConfig(response({ mode: 'cloudflare' })).auth).toEqual({
            mode: 'cloudflare',
        });
        expect(parseServerConfig(response({ mode: 'oauth', oauth: OAUTH_CLIENT })).auth).toEqual({
            mode: 'oauth',
            oauth: OAUTH_CLIENT,
        });
    });

    it('treats an unrecognised mode as the restrictive one', () => {
        // A client that does not understand the server must not open the dashboard: every
        // `mode === …` comparison would otherwise silently answer "no".
        expect(parseServerConfig(response({ mode: 'quantum' })).auth.mode).toBe('oauth');
    });

    it('keeps an OAuth client out of the pass-through modes', () => {
        const config = parseServerConfig(response({ mode: 'cloudflare', oauth: OAUTH_CLIENT }));
        expect(config.auth).toEqual({ mode: 'cloudflare' });
    });

    it('records an OAuth mode with no published client', () => {
        expect(parseServerConfig(response({ mode: 'oauth' })).auth).toEqual({
            mode: 'oauth',
            oauth: null,
        });
    });

    it.each([
        ['zero', 0],
        ['negative', -1],
        ['fractional', 12.5],
        ['not a number', Number.NaN],
    ])('falls back to the default length when the server sends a %s value', (_label, value) => {
        expect(
            shortCodeMaxLength(boot(response({ mode: 'none', short_code_max_length: value }))),
        ).toBe(50);
    });

    it('keeps a valid length', () => {
        expect(
            shortCodeMaxLength(boot(response({ mode: 'none', short_code_max_length: 12 }))),
        ).toBe(12);
    });
});

describe('session transitions', () => {
    it('is booting until both the config and the identity are known', () => {
        expect(isBooting(INITIALIZING)).toBe(true);
        const identifying = boot(response({ mode: 'none' }));
        expect(isBooting(identifying)).toBe(true);
        expect(isAuthorized(identifying)).toBe(false);

        const ready = authReducer(identifying, {
            type: 'identified',
            user: { user_id: 'u1', is_admin: true },
        });
        expect(isBooting(ready)).toBe(false);
        expect(isAuthorized(ready)).toBe(true);
        expect(isAdmin(ready)).toBe(true);
    });

    it('authorizes the pass-through modes with an ambient credential', () => {
        for (const mode of ['none', 'cloudflare'] as const) {
            const state = boot(response({ mode }), null);
            expect(pendingCredential(state)).toEqual({ tag: 'ambient' });
            expect(oauthClient(state)).toBeNull();
        }
    });

    it('leaves OAuth anonymous — and settled — without a token', () => {
        const state = boot(response({ mode: 'oauth', oauth: OAUTH_CLIENT }));
        // Anonymous is a conclusion, not a wait: the sign-in screen must render at once.
        expect(isBooting(state)).toBe(false);
        expect(isAuthorized(state)).toBe(false);
        expect(pendingCredential(state)).toBeNull();
        expect(oauthClient(state)).toEqual(OAUTH_CLIENT);
    });

    it('identifies a restored token', () => {
        const state = boot(response({ mode: 'oauth', oauth: OAUTH_CLIENT }), 'tok');
        expect(pendingCredential(state)).toEqual({ tag: 'bearer', token: 'tok' });
        expect(isBooting(state)).toBe(true);
    });

    it('re-identifies after signing in', () => {
        let state = boot(response({ mode: 'oauth', oauth: OAUTH_CLIENT }));
        state = authReducer(state, { type: 'signedIn', token: 'fresh' });
        expect(pendingCredential(state)).toEqual({ tag: 'bearer', token: 'fresh' });

        state = authReducer(state, { type: 'identified', user: { user_id: 'u', is_admin: false } });
        expect(isAuthorized(state)).toBe(true);
        expect(currentUser(state)).toEqual({ user_id: 'u', is_admin: false });
    });

    it('keeps the credential when the identity probe fails', () => {
        // The API authorises each request on its own, so a failed probe degrades the
        // header and the admin affordances — not access.
        let state = boot(response({ mode: 'oauth', oauth: OAUTH_CLIENT }), 'tok');
        state = authReducer(state, { type: 'identified', user: null });
        expect(isAuthorized(state)).toBe(true);
        expect(currentUser(state)).toBeNull();
        expect(isAdmin(state)).toBe(false);
    });
});

describe('transitions that must not apply', () => {
    it('ignores a second configuration', () => {
        const state = boot(response({ mode: 'none' }));
        const again = authReducer(state, {
            type: 'configured',
            config: parseServerConfig(response({ mode: 'oauth' })),
            token: null,
        });
        expect(again).toBe(state);
    });

    it('ignores sign-out in a mode with no credential to surrender', () => {
        // Dropping "nothing" would strand the app on a sign-in screen the server has no
        // flow for.
        const state = authReducer(boot(response({ mode: 'cloudflare' })), {
            type: 'identified',
            user: null,
        });
        expect(authReducer(state, { type: 'signedOut' })).toBe(state);
    });

    it('does not let a late identity resurrect a closed session', () => {
        let state = boot(response({ mode: 'oauth', oauth: OAUTH_CLIENT }), 'tok');
        state = authReducer(state, { type: 'identified', user: { user_id: 'u', is_admin: true } });
        const signedOut = authReducer(state, { type: 'signedOut' });
        expect(isAuthorized(signedOut)).toBe(false);

        const late = authReducer(signedOut, {
            type: 'identified',
            user: { user_id: 'u', is_admin: true },
        });
        expect(isAuthorized(late)).toBe(false);
        expect(isAdmin(late)).toBe(false);
    });

    it('ignores an identity that arrives before the configuration', () => {
        expect(authReducer(INITIALIZING, { type: 'identified', user: null })).toBe(INITIALIZING);
        expect(authReducer(INITIALIZING, { type: 'signedOut' })).toBe(INITIALIZING);
        expect(authReducer(INITIALIZING, { type: 'signedIn', token: 't' })).toBe(INITIALIZING);
    });
});
