/**
 * The bearer token's only home.
 *
 * `localStorage` is a global mutable channel shared by the request interceptor and the
 * auth state machine; the key naming it was previously written out at both ends, so a
 * rename in one place would have silently signed every user out. Confining it here
 * makes the channel a named module boundary with three total operations.
 */
const TOKEN_KEY = 'auth_token';

/** `null` when no token is stored — the anonymous session, not an error. */
export const readToken = (): string | null => localStorage.getItem(TOKEN_KEY);

export const writeToken = (token: string): void => localStorage.setItem(TOKEN_KEY, token);

export const clearToken = (): void => localStorage.removeItem(TOKEN_KEY);
