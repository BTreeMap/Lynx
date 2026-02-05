import { createContext } from 'react';
import type { UserInfo } from '../types';

export interface AuthContextType {
  authMode: string | null;
  token: string | null;
  userInfo: UserInfo | null;
  isLoading: boolean;
  shortCodeMaxLength: number;
  login: (token: string) => void;
  logout: () => void;
  refreshUserInfo: () => Promise<void>;
}

export const AuthContext = createContext<AuthContextType | undefined>(undefined);
