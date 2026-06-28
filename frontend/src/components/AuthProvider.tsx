import React, { useState, useEffect, useCallback } from 'react';
import type { ReactNode } from 'react';
import { apiClient } from '../api';
import { beginAuthorizationFlow, completeAuthorizationFlow, selectBearerToken } from '../lib/oidc';
import type { OAuthFrontendConfig, UserInfo } from '../types';
import { AuthContext } from '../contexts/AuthContext';

const DEFAULT_SHORT_CODE_MAX_LENGTH = 50;

export const AuthProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [authMode, setAuthMode] = useState<string | null>(null);
  const [token, setToken] = useState<string | null>(localStorage.getItem('auth_token'));
  const [userInfo, setUserInfo] = useState<UserInfo | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [oauthConfig, setOauthConfig] = useState<OAuthFrontendConfig | null>(null);
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
        setOauthConfig(response.oauth ?? null);
        setShortCodeMaxLength(response.short_code_max_length);
      } catch (error) {
        console.error('Failed to fetch auth mode:', error);
        setAuthMode('oauth'); // Default to oauth if unable to fetch
        setOauthConfig(null);
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

  const startOAuthLogin = useCallback(async () => {
    if (authMode !== 'oauth') {
      return;
    }

    if (!oauthConfig) {
      throw new Error('OAuth is not configured on this instance.');
    }

    await beginAuthorizationFlow(oauthConfig);
  }, [authMode, oauthConfig]);

  const completeOAuthLogin = useCallback(
    async (code: string, state: string) => {
      if (!oauthConfig) {
        throw new Error('OAuth is not configured on this instance.');
      }

      const tokenResponse = await completeAuthorizationFlow({
        code,
        state,
        config: oauthConfig,
      });
      const bearerToken = selectBearerToken(tokenResponse);
      login(bearerToken);
    },
    [oauthConfig]
  );

  return (
    <AuthContext.Provider
      value={{
        authMode,
        token,
        userInfo,
        isLoading,
        shortCodeMaxLength,
        oauthConfig,
        login,
        logout,
        startOAuthLogin,
        completeOAuthLogin,
        refreshUserInfo,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
};
