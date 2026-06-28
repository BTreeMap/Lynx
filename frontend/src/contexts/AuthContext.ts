import { createContext } from 'react';
import type { OAuthFrontendConfig, UserInfo } from '../types';

export interface AuthContextType {
  authMode: string | null;
  token: string | null;
  userInfo: UserInfo | null;
  isLoading: boolean;
  shortCodeMaxLength: number;
  oauthConfig: OAuthFrontendConfig | null;
  login: (token: string) => void;
  logout: () => void;
  startOAuthLogin: () => Promise<void>;
  completeOAuthLogin: (code: string, state: string) => Promise<void>;
  refreshUserInfo: () => Promise<void>;
}

export const AuthContext = createContext<AuthContextType | undefined>(undefined);
