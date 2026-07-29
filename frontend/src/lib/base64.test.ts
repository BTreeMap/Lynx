import { describe, expect, it } from 'vitest';
import {
    decodeBase64UrlToBytes,
    decodeBase64UrlToUtf8,
    encodeBytesBase64Url,
    encodeUtf8Base64Url,
} from './base64';

describe('base64url', () => {
    it.each([
        ['ascii', 'abc'],
        ['characters that would break a path', 'a/b?c#d'],
        ['characters that would break a query', 'a+b=c&d'],
        ['non-ascii', 'ünïcødé ✓ 日本語'],
        ['empty', ''],
        ['a single byte', 'x'],
        // Long enough to have overflowed a `String.fromCharCode(...bytes)` spread.
        ['long', 'x'.repeat(200_000)],
    ])('round-trips %s', (_label, value) => {
        expect(decodeBase64UrlToUtf8(encodeUtf8Base64Url(value))).toBe(value);
    });

    it('emits only URL-safe characters and no padding', () => {
        // `?` and `~` are chosen to force both `+` and `/` in the standard alphabet.
        const encoded = encodeUtf8Base64Url('a/b?c#d+e~f');
        expect(encoded).toMatch(/^[A-Za-z0-9\-_]*$/);
    });

    it('accepts its own unpadded output', () => {
        // One code per residue class, so every padding length is exercised.
        for (const code of ['a', 'ab', 'abc', 'abcd']) {
            expect(decodeBase64UrlToUtf8(encodeUtf8Base64Url(code))).toBe(code);
        }
    });

    it('round-trips raw bytes', () => {
        const bytes = Uint8Array.from({ length: 256 }, (_, i) => i);
        expect(decodeBase64UrlToBytes(encodeBytesBase64Url(bytes))).toEqual(bytes);
    });

    describe('decoding untrusted input', () => {
        it('returns null rather than throwing on non-base64', () => {
            expect(decodeBase64UrlToBytes('!!! not base64 !!!')).toBeNull();
            expect(decodeBase64UrlToUtf8('!!! not base64 !!!')).toBeNull();
        });

        it('rejects byte sequences that are not valid UTF-8', () => {
            // 0xFF is not a legal UTF-8 lead byte; a lenient decoder would return U+FFFD.
            const invalid = encodeBytesBase64Url(Uint8Array.of(0xff, 0xfe));
            expect(decodeBase64UrlToBytes(invalid)).not.toBeNull();
            expect(decodeBase64UrlToUtf8(invalid)).toBeNull();
        });
    });
});
