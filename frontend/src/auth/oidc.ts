import { apiClient } from '../api';
import { decodeBase64UrlToUtf8, encodeBytesBase64Url } from '../lib/base64';
import type { OAuthFrontendConfig, OidcDiscoveryResponse, OidcTokenResponse } from '../types';

/**
 * The browser half of the OIDC authorization-code flow with PKCE.
 *
 * Everything here is an effect on browser state (session storage, the address bar) or a
 * network round trip. The state machine that consumes it (`./model.ts`) stays pure.
 */

const OAUTH_STATE_KEY = 'oauth_state';
const OAUTH_NONCE_KEY = 'oauth_nonce';
const OAUTH_VERIFIER_KEY = 'oauth_code_verifier';
const OAUTH_DISCOVERY_KEY = 'oauth_discovery_cache';

type DiscoveryCache = Record<string, OidcDiscoveryResponse>;

const randomString = (byteLength: number): string => {
    const bytes = new Uint8Array(byteLength);
    crypto.getRandomValues(bytes);
    return encodeBytesBase64Url(bytes);
};

const sha256 = async (input: string): Promise<Uint8Array> => {
    const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(input));
    return new Uint8Array(digest);
};

/** A verifier and the challenge derived from it — always constructed as a pair, so a
 *  challenge can never be sent for a verifier that was not stored. */
const createPkcePair = async (): Promise<{ verifier: string; challenge: string }> => {
    const verifier = randomString(96);
    return { verifier, challenge: encodeBytesBase64Url(await sha256(verifier)) };
};

const readDiscoveryCache = (): DiscoveryCache => {
    const raw = sessionStorage.getItem(OAUTH_DISCOVERY_KEY);
    if (!raw) {
        return {};
    }

    try {
        return JSON.parse(raw) as DiscoveryCache;
    } catch {
        // A corrupt cache is indistinguishable from a cold one, and re-fetching is
        // cheap: treat it as absent rather than failing the sign-in.
        return {};
    }
};

const writeDiscoveryCache = (cache: DiscoveryCache): void => {
    sessionStorage.setItem(OAUTH_DISCOVERY_KEY, JSON.stringify(cache));
};

export const getDiscoveryDocument = async (
    issuerUrl: string,
): Promise<OidcDiscoveryResponse> => {
    const cache = readDiscoveryCache();
    const cached = cache[issuerUrl];
    if (cached) {
        return cached;
    }

    const discovery = await apiClient.getOidcDiscovery(issuerUrl);
    writeDiscoveryCache({ ...cache, [issuerUrl]: discovery });
    return discovery;
};

/** Navigates away; the returned promise never resolves in practice. */
export const beginAuthorizationFlow = async (config: OAuthFrontendConfig): Promise<void> => {
    const discovery = await getDiscoveryDocument(config.issuer_url);
    const state = randomString(48);
    const nonce = randomString(48);
    const { verifier, challenge } = await createPkcePair();

    // Written before the navigation, since the redirect back is the only thing that can
    // read them and it happens in a fresh document.
    sessionStorage.setItem(OAUTH_STATE_KEY, state);
    sessionStorage.setItem(OAUTH_NONCE_KEY, nonce);
    sessionStorage.setItem(OAUTH_VERIFIER_KEY, verifier);

    const authUrl = new URL(discovery.authorization_endpoint);
    const query: Readonly<Record<string, string>> = {
        response_type: 'code',
        client_id: config.client_id,
        redirect_uri: config.redirect_uri,
        scope: config.scopes,
        state,
        nonce,
        code_challenge: challenge,
        code_challenge_method: 'S256',
    };
    Object.entries(query).forEach(([key, value]) => authUrl.searchParams.set(key, value));

    window.location.assign(authUrl.toString());
};

/** `null` for anything that is not a JWT with a decodable JSON payload. */
const decodeJwtPayload = (token: string): Record<string, unknown> | null => {
    const segments = token.split('.');
    if (segments.length < 2) {
        return null;
    }

    const json = decodeBase64UrlToUtf8(segments[1]);
    if (json === null) {
        return null;
    }

    try {
        const payload: unknown = JSON.parse(json);
        return typeof payload === 'object' && payload !== null
            ? (payload as Record<string, unknown>)
            : null;
    } catch {
        return null;
    }
};

const extractNonce = (idToken?: string): string | null => {
    const payload = idToken ? decodeJwtPayload(idToken) : null;
    return typeof payload?.nonce === 'string' ? payload.nonce : null;
};

/**
 * Which token the API is given.
 *
 * The backend validates a JWT, so a three-segment access token is preferred; an opaque
 * access token is not one, and the ID token is then the only credential that carries
 * verifiable claims. The final fallback keeps the previous behaviour of sending the
 * access token rather than failing outright.
 */
export const selectBearerToken = (response: OidcTokenResponse): string => {
    if (response.access_token && response.access_token.split('.').length === 3) {
        return response.access_token;
    }
    return response.id_token ?? response.access_token;
};

export const completeAuthorizationFlow = async (params: {
    readonly code: string;
    readonly state: string;
    readonly config: OAuthFrontendConfig;
}): Promise<OidcTokenResponse> => {
    const expectedState = sessionStorage.getItem(OAUTH_STATE_KEY);
    const codeVerifier = sessionStorage.getItem(OAUTH_VERIFIER_KEY);
    const expectedNonce = sessionStorage.getItem(OAUTH_NONCE_KEY);

    if (!expectedState || !codeVerifier) {
        throw new Error('Missing OAuth state. Please retry login.');
    }

    if (expectedState !== params.state) {
        throw new Error('OAuth state mismatch. Please retry login.');
    }

    const discovery = await getDiscoveryDocument(params.config.issuer_url);
    const tokenResponse = await apiClient.exchangeOidcCode({
        tokenEndpoint: discovery.token_endpoint,
        code: params.code,
        clientId: params.config.client_id,
        redirectUri: params.config.redirect_uri,
        codeVerifier,
    });

    const actualNonce = extractNonce(tokenResponse.id_token);
    if (expectedNonce && actualNonce && expectedNonce !== actualNonce) {
        throw new Error('OAuth nonce mismatch. Please retry login.');
    }

    // Single-use by construction: the flow's secrets are dropped as soon as they have
    // been spent, so a replayed callback cannot re-exchange the same verifier.
    sessionStorage.removeItem(OAUTH_STATE_KEY);
    sessionStorage.removeItem(OAUTH_NONCE_KEY);
    sessionStorage.removeItem(OAUTH_VERIFIER_KEY);

    return tokenResponse;
};
