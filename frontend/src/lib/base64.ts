/**
 * base64url codecs (RFC 4648 §5).
 *
 * Built on `btoa`/`atob`, which are Baseline **widely available** — the standard this
 * project targets. The newer `Uint8Array.prototype.toBase64` / `Uint8Array.fromBase64`
 * would be the faster and more direct expression of exactly this, but they are only
 * Baseline *newly available*: the last engine to ship them did so in 2025, and they are
 * absent from runtimes as current as Node 24. Reaching for them today would break the
 * one operation every short link depends on, on browsers that are still well inside
 * support. Switch to them — and delete the helpers below — once they are widely
 * available; nothing else in the app needs to change.
 *
 * What this deliberately is *not* is a polyfill: no built-in prototype is patched, and
 * no capability is claimed that the platform does not have. These are two module
 * functions over an API that has existed for a decade.
 *
 * Encoding is total. Decoding is partial by nature — its input is a route parameter or
 * a JWT segment — so it returns `null` instead of throwing, and callers eliminate the
 * absence explicitly.
 */

/**
 * Byte-per-character string, the form `btoa` accepts.
 *
 * A loop rather than `String.fromCharCode(...bytes)`: the spread form pushes one
 * argument per byte onto the call stack and throws on large inputs. Everything encoded
 * here is small (a short code, a 96-byte PKCE verifier, a 32-byte digest), but a codec
 * that fails on size would be a trap for the next caller.
 */
const toBinaryString = (bytes: Uint8Array): string => {
    let binary = '';
    for (const byte of bytes) {
        binary += String.fromCharCode(byte);
    }
    return binary;
};

/** Standard base64 → base64url, with padding dropped (it is recoverable from length). */
const toUrlAlphabet = (base64: string): string =>
    base64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

/** base64url → standard base64, restoring the padding `atob` requires. */
const toStandardAlphabet = (base64Url: string): string => {
    const standard = base64Url.replace(/-/g, '+').replace(/_/g, '/');
    return standard + '='.repeat((4 - (standard.length % 4)) % 4);
};

export const encodeBytesBase64Url = (bytes: Uint8Array): string =>
    toUrlAlphabet(btoa(toBinaryString(bytes)));

export const encodeUtf8Base64Url = (text: string): string =>
    encodeBytesBase64Url(new TextEncoder().encode(text));

/** `null` when the input is not valid base64url. Padding is optional. */
export const decodeBase64UrlToBytes = (text: string): Uint8Array | null => {
    try {
        const binary = atob(toStandardAlphabet(text));
        return Uint8Array.from(binary, (char) => char.charCodeAt(0));
    } catch {
        return null;
    }
};

/** `null` when the input is not valid base64url *or* not valid UTF-8. */
export const decodeBase64UrlToUtf8 = (text: string): string | null => {
    const bytes = decodeBase64UrlToBytes(text);
    if (bytes === null) {
        return null;
    }
    try {
        // `fatal` rejects malformed sequences instead of substituting U+FFFD: a short
        // code that does not round-trip is a decoding failure, not a code that happens
        // to contain replacement characters.
        return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    } catch {
        return null;
    }
};
