import React, { createContext, useContext, useState, useEffect } from 'react';
import type { ReactNode } from 'react';
import { apiClient } from './api';
import type { UserInfo } from './types';

interface AuthContextType {
  token: string | null;
  userInfo: UserInfo | null;
  isLoading: boolean;
  login: (token: string) => void;
  logout: () => void;
  refreshUserInfo: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export const AuthProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [token, setToken] = useState<string | null>(localStorage.getItem('auth_token'));
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const refreshUserInfo = async () => {
    if (token) {
      try {
        const info = await apiClient.getUserInfo();
        setUserInfo(info);
      } catch (error) {
        console.error('Failed to fetch user info:', error);
        setUserInfo(null);
      }
    }
  };

  useEffect(() => {
    const loadUserInfo = async () => {
      setIsLoading(true);
      if (token) {
        await refreshUserInfo();
      }
      setIsLoading(false);
    };
    loadUserInfo();
  }, [token]);

  const login = (newToken: string) => {
    localStorage.setItem('auth_token', newToken);
    setToken(newToken);
  };

  const logout = () => {
    localStorage.removeItem('auth_token');
    setToken(null);
    setUserInfo(null);
  };

  return (
    <AuthContext.Provider value={{ token, userInfo, isLoading, login, logout, refreshUserInfo }}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (context === undefined) {
    throw new Error('useAuth must be used within an AuthProvider');
  }
  return context;
};
