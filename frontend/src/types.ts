/**
 * Wire types for the Lynx API.
 *
 * These describe what the server *says* it sends. They are declarations, not
 * guarantees: TypeScript erases them, so nothing here validates a payload. Values that
 * drive control flow — the auth mode, a short code from the URL bar — are parsed into
 * domain types at a single boundary before use (`src/auth/model.ts`,
 * `src/utils/url.ts`); the rest are rendered as data and are safe to take at face
 * value.
 *
 * Every field is `readonly`: a response is a value, and nothing in the app mutates one
 * in place. The modifier costs nothing at runtime and turns an accidental write into a
 * compile error.
 */

export interface ShortenedUrl {
    readonly id: number;
    readonly short_code: string;
    readonly original_url: string;
    readonly created_at: number;
    readonly created_by: string | null;
    readonly clicks: number;
    readonly is_active: boolean;
    readonly redirect_base_url?: string | null;
}

export interface PaginatedUrlsResponse {
    readonly urls: readonly ShortenedUrl[];
    readonly next_cursor?: string | null;
    readonly has_more: boolean;
}

export interface CreateUrlRequest {
    readonly url: string;
    readonly custom_code?: string;
}

export interface UrlHistoryEntry {
    readonly id: number;
    readonly short_code: string;
    readonly historic_url: string;
    readonly changed_at: number;
    readonly changed_by: string | null;
}

export interface UserInfo {
    readonly user_id: string | null;
    readonly is_admin: boolean;
}

export interface AuthModeResponse {
    readonly mode: string;
    readonly short_code_max_length: number;
    readonly oauth?: OAuthFrontendConfig;
}

export interface OAuthFrontendConfig {
    readonly issuer_url: string;
    readonly client_id: string;
    readonly scopes: string;
    readonly redirect_uri: string;
}

export interface OidcDiscoveryResponse {
    readonly authorization_endpoint: string;
    readonly token_endpoint: string;
}

export interface OidcTokenResponse {
    readonly access_token: string;
    readonly token_type: string;
    readonly expires_in?: number;
    readonly id_token?: string;
    readonly scope?: string;
    readonly refresh_token?: string;
}

export interface SuccessResponse {
    readonly message: string;
}

export interface AnalyticsEntry {
    readonly id: number;
    readonly short_code: string;
    readonly time_bucket: number;
    readonly country_code: string | null;
    readonly region: string | null;
    readonly city: string | null;
    readonly asn: number | null;
    readonly ip_version: number;
    readonly visit_count: number;
    readonly created_at: number;
    readonly updated_at: number;
}

export interface AnalyticsResponse {
    readonly entries: readonly AnalyticsEntry[];
    readonly total: number;
    readonly clicks: number;
}

export interface AnalyticsAggregate {
    readonly dimension: string;
    readonly visit_count: number;
}

export interface AnalyticsAggregateResponse {
    readonly aggregates: readonly AnalyticsAggregate[];
    readonly total: number;
    readonly clicks: number;
}

export interface SearchParams {
    readonly q: string;
    readonly created_by?: string;
    readonly created_from?: number;
    readonly created_to?: number;
    readonly is_active?: boolean;
    readonly limit?: number;
    readonly cursor?: string;
}

export interface SearchResponse {
    readonly items: readonly ShortenedUrl[];
    readonly next_cursor?: string | null;
    readonly has_more: boolean;
}
