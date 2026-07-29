import axios from 'axios';
import type {
    AnalyticsAggregateResponse,
    AnalyticsResponse,
    AuthModeResponse,
    CreateUrlRequest,
    OidcDiscoveryResponse,
    OidcTokenResponse,
    PaginatedUrlsResponse,
    SearchParams,
    SearchResponse,
    ShortenedUrl,
    SuccessResponse,
    UrlHistoryEntry,
    UserInfo,
} from './types';
import { readToken } from './auth/tokenStore';
import { encodeShortCodeForApi, normalizeOriginalUrl } from './utils/url';

/**
 * The app's only HTTP surface. Components never call `axios` or `fetch`: the base URL,
 * bearer injection, short-code encoding and destination normalisation all live here, so
 * a call site cannot get one of them wrong.
 */

const API_BASE_URL = import.meta.env.VITE_API_URL || '/api';

const api = axios.create({ baseURL: API_BASE_URL });

/** Separate instance: the identity provider is a third party and must not receive the
 *  Lynx bearer token that the interceptor below attaches. */
const oidcClient = axios.create();

api.interceptors.request.use((config) => {
    const token = readToken();
    if (token) {
        config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
});

/**
 * Every read accepts an `AbortSignal`. Reads are keyed on values the user changes while
 * they are still in flight — a route parameter, a search query, an analytics dimension —
 * and a response that outlives its key must be cancelled rather than allowed to land on
 * a screen that has moved on.
 */
export interface RequestOptions {
    readonly signal?: AbortSignal;
}

export interface ListOptions extends RequestOptions {
    readonly limit?: number;
    readonly cursor?: string;
}

export interface AnalyticsOptions extends RequestOptions {
    readonly startTime?: number;
    readonly endTime?: number;
    readonly limit?: number;
}

export interface AggregateOptions extends AnalyticsOptions {
    readonly groupBy?: string;
}

/*
  Query parameters are passed as one object with `undefined` holes rather than assembled
  by a chain of `if (x !== undefined) params.x = x`. Axios omits `undefined` entries when
  it serialises (it renders `null` as an empty string, which is why absence is spelled
  `undefined` throughout). The conditionals were re-implementing that rule once per
  endpoint.
*/

export const apiClient = {
    async getAuthMode(options: RequestOptions = {}): Promise<AuthModeResponse> {
        const { data } = await api.get<AuthModeResponse>('/auth/mode', options);
        return data;
    },

    async getOidcDiscovery(
        issuerUrl: string,
        options: RequestOptions = {},
    ): Promise<OidcDiscoveryResponse> {
        const discoveryUrl = `${issuerUrl.replace(/\/$/, '')}/.well-known/openid-configuration`;
        const { data } = await oidcClient.get<OidcDiscoveryResponse>(discoveryUrl, options);
        return data;
    },

    async exchangeOidcCode(params: {
        readonly tokenEndpoint: string;
        readonly code: string;
        readonly clientId: string;
        readonly redirectUri: string;
        readonly codeVerifier: string;
    }): Promise<OidcTokenResponse> {
        const body = new URLSearchParams({
            grant_type: 'authorization_code',
            code: params.code,
            client_id: params.clientId,
            redirect_uri: params.redirectUri,
            code_verifier: params.codeVerifier,
        });

        const { data } = await oidcClient.post<OidcTokenResponse>(params.tokenEndpoint, body, {
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        });
        return data;
    },

    async getUserInfo(options: RequestOptions = {}): Promise<UserInfo> {
        const { data } = await api.get<UserInfo>('/user/info', options);
        return data;
    },

    async createUrl(request: CreateUrlRequest): Promise<ShortenedUrl> {
        const { data } = await api.post<ShortenedUrl>('/urls', {
            ...request,
            url: normalizeOriginalUrl(request.url),
        });
        return data;
    },

    async getUrl(code: string, options: RequestOptions = {}): Promise<ShortenedUrl> {
        const { data } = await api.get<ShortenedUrl>(
            `/urls/${encodeShortCodeForApi(code)}`,
            options,
        );
        return data;
    },

    async updateUrl(code: string, url: string): Promise<ShortenedUrl> {
        const { data } = await api.patch<ShortenedUrl>(`/urls/${encodeShortCodeForApi(code)}`, {
            url: normalizeOriginalUrl(url),
        });
        return data;
    },

    async getUrlHistory(
        code: string,
        options: RequestOptions = {},
    ): Promise<readonly UrlHistoryEntry[]> {
        const { data } = await api.get<UrlHistoryEntry[]>(
            `/urls/${encodeShortCodeForApi(code)}/history`,
            options,
        );
        return data;
    },

    async restoreUrl(code: string, historyId: number): Promise<ShortenedUrl> {
        const { data } = await api.post<ShortenedUrl>(
            `/urls/${encodeShortCodeForApi(code)}/history/${historyId}/restore`,
            {},
        );
        return data;
    },

    async listUrls({ limit = 50, cursor, signal }: ListOptions = {}): Promise<PaginatedUrlsResponse> {
        const { data } = await api.get<PaginatedUrlsResponse>('/urls', {
            params: { limit, cursor },
            signal,
        });
        return data;
    },

    async searchUrls(
        params: SearchParams,
        { signal }: RequestOptions = {},
    ): Promise<SearchResponse> {
        const { data } = await api.get<SearchResponse>('/urls/search', { params, signal });
        return data;
    },

    async deactivateUrl(code: string): Promise<SuccessResponse> {
        const { data } = await api.put<SuccessResponse>(
            `/urls/${encodeShortCodeForApi(code)}/deactivate`,
            {},
        );
        return data;
    },

    async reactivateUrl(code: string): Promise<SuccessResponse> {
        const { data } = await api.put<SuccessResponse>(
            `/urls/${encodeShortCodeForApi(code)}/reactivate`,
        );
        return data;
    },

    async getAnalytics(
        code: string,
        { startTime, endTime, limit = 100, signal }: AnalyticsOptions = {},
    ): Promise<AnalyticsResponse> {
        const { data } = await api.get<AnalyticsResponse>(
            `/analytics/${encodeShortCodeForApi(code)}`,
            { params: { start_time: startTime, end_time: endTime, limit }, signal },
        );
        return data;
    },

    async getAnalyticsAggregate(
        code: string,
        { groupBy = 'country', startTime, endTime, limit = 100, signal }: AggregateOptions = {},
    ): Promise<AnalyticsAggregateResponse> {
        const { data } = await api.get<AnalyticsAggregateResponse>(
            `/analytics/${encodeShortCodeForApi(code)}/aggregate`,
            {
                params: { group_by: groupBy, start_time: startTime, end_time: endTime, limit },
                signal,
            },
        );
        return data;
    },
};
