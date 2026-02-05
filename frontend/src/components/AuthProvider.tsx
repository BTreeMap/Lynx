import React, { useState, useEffect, useCallback } from 'react';
import type { ReactNode } from 'react';
import { apiClient } from '../api';
import type { UserInfo } from '../types';
import { AuthContext } from '../contexts/AuthContext';

const DEFAULT_SHORT_CODE_MAX_LENGTH = 50;

export const AuthProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [authMode, setAuthMode] = useState<string | null>(null);
  const [token, setToken] = useState<string | null>(localStorage.getItem('auth_token'));
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [shortCodeMaxLength, setShortCodeMaxLength] = useState<number>(
    DEFAULT_SHORT_CODE_MAX_LENGTH
  );

  const refreshUserInfo = useCallback(async () => {
    if (token || authMode === 'none' || authMode === 'cloudflare') {
      try {
        const info = await apiClient.getUserInfo();
        setUserInfo(info);
      } catch (error) {
        console.error('Failed to fetch user info:', error);
        setUserInfo(null);
      }
    }
  }, [token, authMode]);

  // Fetch auth mode on mount
  useEffect(() => {
    const fetchAuthMode = async () => {
      try {
        const response = await apiClient.getAuthMode();
        setAuthMode(response.mode);
        setShortCodeMaxLength(response.short_code_max_length);
      } catch (error) {
        console.error('Failed to fetch auth mode:', error);
        setAuthMode('oauth'); // Default to oauth if unable to fetch
        setShortCodeMaxLength(DEFAULT_SHORT_CODE_MAX_LENGTH);
      }
    };
    fetchAuthMode();
  }, []);

  useEffect(() => {
    const loadUserInfo = async () => {
      if (authMode === null) {
        // Wait for auth mode to be loaded
        return;
      }

      setIsLoading(true);
      
      // For auth=none or cloudflare, we don't need a token
      if (authMode === 'none' || authMode === 'cloudflare') {
        await refreshUserInfo();
      } else if (token) {
        // For oauth, we need a token
        await refreshUserInfo();
      }
      
      setIsLoading(false);
    };
    loadUserInfo();
  }, [token, authMode, refreshUserInfo]);

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
    <AuthContext.Provider value={{ authMode, token, userInfo, isLoading, shortCodeMaxLength, login, logout, refreshUserInfo }}>
      {children}
    </AuthContext.Provider>
  );
};
