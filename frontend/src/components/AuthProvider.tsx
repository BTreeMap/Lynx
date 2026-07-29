import React, { useCallback, useEffect, useMemo, useReducer } from 'react';
import type { ReactNode } from 'react';
import { apiClient } from '../api';
import {
    authReducer,
    FALLBACK_SERVER_CONFIG,
    INITIALIZING,
    oauthClient,
    parseServerConfig,
    pendingCredential,
} from '../auth/model';
import { beginAuthorizationFlow, completeAuthorizationFlow, selectBearerToken } from '../auth/oidc';
import { clearToken, readToken, writeToken } from '../auth/tokenStore';
import { AuthContext, type AuthContextValue } from '../contexts/AuthContext';

/**
 * Interprets the authentication machine against the network and browser storage.
 *
 * The reducer decides which transitions exist; this decides when a request goes out.
 * Both effects below are *synchronisations* — "read the server's configuration", "read
 * the identity this credential names" — which is what an effect is for. Everything
 * caused by a user action happens instead inside the command that action invokes.
 */
export const AuthProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
    const [state, dispatch] = useReducer(authReducer, INITIALIZING);

    // Read the server's auth configuration exactly once, on mount.
    useEffect(() => {
        const controller = new AbortController();
        apiClient.getAuthMode({ signal: controller.signal }).then(
            (response) => {
                if (controller.signal.aborted) return;
                dispatch({
                    type: 'configured',
                    config: parseServerConfig(response),
                    token: readToken(),
                });
            },
            (error: unknown) => {
                if (controller.signal.aborted) return;
                console.error('Failed to fetch auth mode:', error);
                // Boot into the restrictive default rather than staying on the splash
                // screen: an unreachable probe must not become a hung app.
                dispatch({
                    type: 'configured',
                    config: FALLBACK_SERVER_CONFIG,
                    token: readToken(),
                });
            },
        );
        return () => controller.abort();
    }, []);

    /*
      The identity probe's dependency is the credential itself, flattened to a string so
      that the effect re-runs on a *different* credential and never merely on a new
      object. `null` means no probe is outstanding — the machine is still configuring,
      or anonymous, or already identified.
    */
    const credential = pendingCredential(state);
    const credentialKey =
        credential === null
            ? null
            : credential.tag === 'bearer'
              ? `bearer:${credential.token}`
              : 'ambient';

    useEffect(() => {
        if (credentialKey === null) return;
        const controller = new AbortController();
        apiClient.getUserInfo({ signal: controller.signal }).then(
            (user) => {
                if (!controller.signal.aborted) dispatch({ type: 'identified', user });
            },
            (error: unknown) => {
                if (controller.signal.aborted) return;
                console.error('Failed to fetch user info:', error);
                // The credential stands even when the identity behind it cannot be read:
                // the API authorises every request on its own, so a failed probe degrades
                // the header and the admin affordances, not access itself.
                dispatch({ type: 'identified', user: null });
            },
        );
        return () => controller.abort();
    }, [credentialKey]);

    const client = oauthClient(state);

    const signOut = useCallback(() => {
        clearToken();
        dispatch({ type: 'signedOut' });
    }, []);

    const beginOAuthSignIn = useCallback(async () => {
        if (!client) {
            throw new Error('OAuth is not configured on this instance.');
        }
        await beginAuthorizationFlow(client);
    }, [client]);

    const completeOAuthSignIn = useCallback(
        async (code: string, oauthState: string) => {
            if (!client) {
                throw new Error('OAuth is not configured on this instance.');
            }
            const tokenResponse = await completeAuthorizationFlow({
                code,
                state: oauthState,
                config: client,
            });
            const token = selectBearerToken(tokenResponse);
            // Storage first: the interceptor reads it there, and the dispatch below is
            // what triggers the identity probe that will immediately need it.
            writeToken(token);
            dispatch({ type: 'signedIn', token });
        },
        [client],
    );

    const value = useMemo<AuthContextValue>(
        () => ({ state, signOut, beginOAuthSignIn, completeOAuthSignIn }),
        [state, signOut, beginOAuthSignIn, completeOAuthSignIn],
    );

    return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};
