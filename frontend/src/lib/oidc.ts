import { apiClient } from '../api';
import type {
  OidcDiscoveryResponse,
  OidcTokenResponse,
  OAuthFrontendConfig,
} from '../types';

const OAUTH_STATE_KEY = 'oauth_state';
const OAUTH_NONCE_KEY = 'oauth_nonce';
const OAUTH_VERIFIER_KEY = 'oauth_code_verifier';
const OAUTH_DISCOVERY_KEY = 'oauth_discovery_cache';

type DiscoveryCache = Record<string, OidcDiscoveryResponse>;

function randomString(length = 64): string {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);
  return base64UrlEncode(bytes);
}

function base64UrlEncode(input: Uint8Array): string {
  let binary = '';
  input.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

function stringToArrayBuffer(value: string): ArrayBuffer {
  const view = new TextEncoder().encode(value);
  return view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength);
}

async function sha256(input: string): Promise<Uint8Array> {
  const digest = await crypto.subtle.digest('SHA-256', stringToArrayBuffer(input));
  return new Uint8Array(digest);
}

export async function createPkcePair(): Promise<{ verifier: string; challenge: string }> {
  const verifier = randomString(96);
  const challenge = base64UrlEncode(await sha256(verifier));
  return { verifier, challenge };
}

function readDiscoveryCache(): DiscoveryCache {
  const raw = sessionStorage.getItem(OAUTH_DISCOVERY_KEY);
  if (!raw) {
    return {};
  }

  try {
    return JSON.parse(raw) as DiscoveryCache;
  } catch {
    return {};
  }
}

function writeDiscoveryCache(cache: DiscoveryCache): void {
  sessionStorage.setItem(OAUTH_DISCOVERY_KEY, JSON.stringify(cache));
}

export async function getDiscoveryDocument(issuerUrl: string): Promise<OidcDiscoveryResponse> {
  const cache = readDiscoveryCache();
  if (cache[issuerUrl]) {
    return cache[issuerUrl];
  }

  const discovery = await apiClient.getOidcDiscovery(issuerUrl);
  cache[issuerUrl] = discovery;
  writeDiscoveryCache(cache);
  return discovery;
}

export async function beginAuthorizationFlow(config: OAuthFrontendConfig): Promise<void> {
  const discovery = await getDiscoveryDocument(config.issuer_url);
  const state = randomString(48);
  const nonce = randomString(48);
  const { verifier, challenge } = await createPkcePair();

  sessionStorage.setItem(OAUTH_STATE_KEY, state);
  sessionStorage.setItem(OAUTH_NONCE_KEY, nonce);
  sessionStorage.setItem(OAUTH_VERIFIER_KEY, verifier);

  const authUrl = new URL(discovery.authorization_endpoint);
  authUrl.searchParams.set('response_type', 'code');
  authUrl.searchParams.set('client_id', config.client_id);
  authUrl.searchParams.set('redirect_uri', config.redirect_uri);
  authUrl.searchParams.set('scope', config.scopes);
  authUrl.searchParams.set('state', state);
  authUrl.searchParams.set('nonce', nonce);
  authUrl.searchParams.set('code_challenge', challenge);
  authUrl.searchParams.set('code_challenge_method', 'S256');

  window.location.assign(authUrl.toString());
}

function decodeJwtPayload(token: string): Record<string, unknown> {
  const segments = token.split('.');
  if (segments.length < 2) {
    throw new Error('Invalid JWT format');
  }

  const base64 = segments[1].replace(/-/g, '+').replace(/_/g, '/');
  const padded = base64 + '='.repeat((4 - (base64.length % 4)) % 4);
  const json = atob(padded);
  return JSON.parse(json) as Record<string, unknown>;
}

function extractNonce(idToken?: string): string | null {
  if (!idToken) {
    return null;
  }

  try {
    const payload = decodeJwtPayload(idToken);
    return typeof payload.nonce === 'string' ? payload.nonce : null;
  } catch {
    return null;
  }
}

export function selectBearerToken(response: OidcTokenResponse): string {
  if (response.access_token && response.access_token.split('.').length === 3) {
    return response.access_token;
  }

  if (response.id_token) {
    return response.id_token;
  }

  return response.access_token;
}

export async function completeAuthorizationFlow(params: {
  code: string;
  state: string;
  config: OAuthFrontendConfig;
}): Promise<OidcTokenResponse> {
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

  sessionStorage.removeItem(OAUTH_STATE_KEY);
  sessionStorage.removeItem(OAUTH_NONCE_KEY);
  sessionStorage.removeItem(OAUTH_VERIFIER_KEY);

  return tokenResponse;
}
