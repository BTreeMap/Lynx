import axios from 'axios';
import type { ShortenedUrl, CreateUrlRequest, UserInfo, SuccessResponse } from './types';

const API_BASE_URL = import.meta.env.VITE_API_URL || 'http://localhost:8080';

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
  async getUserInfo(): Promise<UserInfo> {
    const { data } = await api.get<UserInfo>('/user/info');
    return data;
  },

  async createUrl(request: CreateUrlRequest): Promise<ShortenedUrl> {
    const { data } = await api.post<ShortenedUrl>('/urls', request);
    return data;
  },

  async getUrl(code: string): Promise<ShortenedUrl> {
    const { data } = await api.get<ShortenedUrl>(`/urls/${code}`);
    return data;
  },

  async listUrls(limit = 50, offset = 0): Promise<ShortenedUrl[]> {
    const { data } = await api.get<ShortenedUrl[]>('/urls', {
      params: { limit, offset },
    });
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
};
