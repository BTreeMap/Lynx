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
}

export interface ErrorResponse {
  error: string;
}

export interface SuccessResponse {
  message: string;
}
