export interface ShortenedUrl {
  id: number;
  short_code: string;
  original_url: string;
  created_at: number;
  created_by: string | null;
  clicks: number;
  is_active: boolean;
  redirect_base_url?: string | null;
}

export interface PaginatedUrlsResponse {
  urls: ShortenedUrl[];
  next_cursor?: string | null;
  has_more: boolean;
}

export interface CreateUrlRequest {
  url: string;
  custom_code?: string;
}

export interface UserInfo {
  user_id: string | null;
  is_admin: boolean;
}

export interface AuthModeResponse {
  mode: string;
  short_code_max_length?: number;
}

export interface ErrorResponse {
  error: string;
}

export interface SuccessResponse {
  message: string;
}

export interface AnalyticsEntry {
  id: number;
  short_code: string;
  time_bucket: number;
  country_code: string | null;
  region: string | null;
  city: string | null;
  asn: number | null;
  ip_version: number;
  visit_count: number;
  created_at: number;
  updated_at: number;
}

export interface AnalyticsResponse {
  entries: AnalyticsEntry[];
  total: number;
  clicks: number;
}

export interface AnalyticsAggregate {
  dimension: string;
  visit_count: number;
}

export interface AnalyticsAggregateResponse {
  aggregates: AnalyticsAggregate[];
  total: number;
  clicks: number;
}

export interface SearchParams {
  q: string;
  created_by?: string;
  created_from?: number;
  created_to?: number;
  is_active?: boolean;
  limit?: number;
  cursor?: string;
}

export interface SearchResponse {
  items: ShortenedUrl[];
  next_cursor?: string | null;
  has_more: boolean;
}
