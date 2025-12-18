import axios from 'axios';
import type { ShortenedUrl, CreateUrlRequest, UserInfo, SuccessResponse, AuthModeResponse, PaginatedUrlsResponse, AnalyticsResponse, AnalyticsAggregateResponse, SearchParams, SearchResponse } from './types';
import { normalizeOriginalUrl } from './utils/url';

const API_BASE_URL = import.meta.env.VITE_API_URL || '/api';

const api = axios.create({
  baseURL: API_BASE_URL,
});

// Add token to all requests if available
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('auth_token');
  if (token) {
    config.headers.Authorization = `Bearer ${token}`;
  }
  return config;
});

export const apiClient = {
  async getAuthMode(): Promise<AuthModeResponse> {
    const { data } = await api.get<AuthModeResponse>('/auth/mode');
    return data;
  },

  async getUserInfo(): Promise<UserInfo> {
    const { data } = await api.get<UserInfo>('/user/info');
    return data;
  },

  async createUrl(request: CreateUrlRequest): Promise<ShortenedUrl> {
    const normalizedRequest: CreateUrlRequest = {
      ...request,
      url: normalizeOriginalUrl(request.url),
    };

    const { data } = await api.post<ShortenedUrl>('/urls', normalizedRequest);
    return data;
  },

  async getUrl(code: string): Promise<ShortenedUrl> {
    const { data } = await api.get<ShortenedUrl>(`/urls/${code}`);
    return data;
  },

  async listUrls(limit = 50, cursor?: string): Promise<PaginatedUrlsResponse> {
    const params: { limit: number; cursor?: string } = { limit };
    if (cursor) {
      params.cursor = cursor;
    }
    const { data } = await api.get<PaginatedUrlsResponse>('/urls', { params });
    return data;
  },

  async deactivateUrl(code: string): Promise<SuccessResponse> {
    const { data } = await api.put<SuccessResponse>(`/urls/${code}/deactivate`, {});
    return data;
  },

  async reactivateUrl(code: string): Promise<SuccessResponse> {
    const { data } = await api.put<SuccessResponse>(`/urls/${code}/reactivate`);
    return data;
  },

  async healthCheck(): Promise<SuccessResponse> {
    const { data } = await api.get<SuccessResponse>('/health');
    return data;
  },

  async getAnalytics(code: string, startTime?: number, endTime?: number, limit = 100): Promise<AnalyticsResponse> {
    const params: { start_time?: number; end_time?: number; limit: number } = { limit };
    if (startTime !== undefined) params.start_time = startTime;
    if (endTime !== undefined) params.end_time = endTime;
    const { data } = await api.get<AnalyticsResponse>(`/analytics/${code}`, { params });
    return data;
  },

  async getAnalyticsAggregate(code: string, groupBy = 'country', startTime?: number, endTime?: number, limit = 100): Promise<AnalyticsAggregateResponse> {
    const params: { group_by: string; start_time?: number; end_time?: number; limit: number } = { group_by: groupBy, limit };
    if (startTime !== undefined) params.start_time = startTime;
    if (endTime !== undefined) params.end_time = endTime;
    const { data } = await api.get<AnalyticsAggregateResponse>(`/analytics/${code}/aggregate`, { params });
    return data;
  },

  async searchUrls(searchParams: SearchParams): Promise<SearchResponse> {
    const params: Record<string, string | number | boolean> = { q: searchParams.q };
    if (searchParams.created_by !== undefined) params.created_by = searchParams.created_by;
    if (searchParams.created_from !== undefined) params.created_from = searchParams.created_from;
    if (searchParams.created_to !== undefined) params.created_to = searchParams.created_to;
    if (searchParams.is_active !== undefined) params.is_active = searchParams.is_active;
    if (searchParams.limit !== undefined) params.limit = searchParams.limit;
    if (searchParams.cursor !== undefined) params.cursor = searchParams.cursor;
    const { data } = await api.get<SearchResponse>('/urls/search', { params });
    return data;
  },
};
