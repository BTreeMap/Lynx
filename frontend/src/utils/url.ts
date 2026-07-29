import { decodeBase64UrlToUtf8, encodeUtf8Base64Url } from '../lib/base64';

const ensureTrailingSlash = (value: string) => (value.endsWith('/') ? value : `${value}/`);

/** Build-time fallback base, or `undefined` when unset or blank. Resolved once: the
 *  value is inlined at build time and cannot change while the app is running. */
const envRedirectBase = ((raw: string | undefined): string | undefined => {
    const trimmed = raw?.trim();
    return trimmed ? trimmed : undefined;
})(import.meta.env.VITE_REDIRECT_URL);

/**
 * Absolute short link, or `null` when no base is known — a link the app cannot address
 * is an expected outcome (an instance that publishes no redirect base), not an error,
 * and every caller renders something else for it.
 */
export const buildShortLink = (code: string, candidateBase?: string | null): string | null => {
    const base = (candidateBase ?? '').trim() || envRedirectBase;
    if (!base) {
        return null;
    }

    try {
        return new URL(code, ensureTrailingSlash(base)).toString();
    } catch (error) {
        console.warn('Failed to construct short link', { base, code, error });
        return null;
    }
};

/**
 * Canonicalise a destination through the URL parser, leaving unparseable input for the
 * server to reject. Applied once, inside `apiClient`, so create and update cannot
 * normalise differently.
 */
export const normalizeOriginalUrl = (value: string): string => {
    const trimmed = value.trim();
    if (!trimmed) {
        return trimmed;
    }

    try {
        return new URL(trimmed).toString();
    } catch {
        return trimmed;
    }
};

/**
 * Short codes travel base64url-encoded in paths: a code may contain `/`, `?` or `#`,
 * which percent-encoding alone does not reliably survive intact through every proxy.
 */
export const encodeShortCodeForApi = (value: string): string => encodeUtf8Base64Url(value);

/**
 * The inverse, over untrusted input: the encoded code arrives from the address bar and
 * may be anything at all. `null` is the parse failure, which the route renders as an
 * invalid link rather than issuing a request for a code it could not read.
 */
export const decodeShortCodeFromApi = (value: string): string | null =>
    decodeBase64UrlToUtf8(value);
