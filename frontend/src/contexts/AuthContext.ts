import { createContext } from 'react';
import type { AuthState } from '../auth/model';

/**
 * The authentication state plus the three commands that can change it.
 *
 * State is exposed as one union rather than as the six loosely-related fields it used
 * to be (`authMode`, `token`, `userInfo`, `isLoading`, `shortCodeMaxLength`,
 * `oauthConfig`); consumers ask the selectors in `src/auth/model.ts` for what they
 * need. Commands are the only way to advance it, so no consumer can drive the session
 * into a state the reducer forbids.
 */
export interface AuthContextValue {
    readonly state: AuthState;
    /** Drops the bearer credential. A no-op in the pass-through modes, which have none. */
    readonly signOut: () => void;
    /** Redirects to the identity provider. Rejects if this instance has no OAuth client. */
    readonly beginOAuthSignIn: () => Promise<void>;
    /** Exchanges an authorization code for a token and adopts it. */
    readonly completeOAuthSignIn: (code: string, state: string) => Promise<void>;
}

export const AuthContext = createContext<AuthContextValue | undefined>(undefined);
