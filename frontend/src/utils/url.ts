const ensureTrailingSlash = (value: string) => (value.endsWith('/') ? value : `${value}/`);

type ImportMetaWithEnv = ImportMeta & {
    env?: Record<string, string | undefined>;
};

const resolveEnvRedirectBase = (): string | undefined => {
    const meta = import.meta as ImportMetaWithEnv;
    const raw = meta.env?.VITE_REDIRECT_URL ?? (meta as unknown as { env?: Record<string, string | undefined> }).env?.VITE_REDIRECT_URL;
    if (typeof raw !== 'string') {
        return undefined;
    }
    const trimmed = raw.trim();
    return trimmed.length > 0 ? trimmed : undefined;
};

export const buildShortLink = (code: string, candidateBase?: string | null): string | null => {
    const fallback = resolveEnvRedirectBase();
    const base = (candidateBase ?? '').trim() || fallback;
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

export const getRedirectBase = () => resolveEnvRedirectBase();

export const normalizeOriginalUrl = (value: string): string => {
    const trimmed = value.trim();
    if (!trimmed) {
        return trimmed;
    }

    try {
        const parsed = new URL(trimmed);
        return parsed.toString();
    } catch {
        return trimmed;
    }
};


export const encodeShortCodeForApi = (value: string): string => {
    const bytes = new TextEncoder().encode(value);
    return bytes.toBase64({ alphabet: 'base64url', omitPadding: true });
};

export const decodeShortCodeFromApi = (value: string): string => {
    const normalized = value.replace(/-/g, '+').replace(/_/g, '/');
    const paddingLength = (4 - (normalized.length % 4)) % 4;
    const padded = normalized + '='.repeat(paddingLength);
    const binary = atob(padded);
    const bytes = Uint8Array.from(binary, (char) => char.charCodeAt(0));
    return new TextDecoder().decode(bytes);
};
