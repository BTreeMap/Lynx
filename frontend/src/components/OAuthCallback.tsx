import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { authMode } from '../auth/model';
import { assertNever } from '../lib/assertNever';
import { useAuth } from '../hooks/useAuth';
import { Logo } from './layout/Logo';
import { Spinner } from './ui/Spinner';

/**
 * What the identity provider sent back, parsed out of the query string before anything
 * acts on it. The three outcomes are mutually exclusive, so the exchange path receives
 * a code and a state that are known to be present instead of two nullable lookups it
 * has to re-check.
 */
type CallbackParams =
    | { readonly tag: 'granted'; readonly code: string; readonly state: string }
    | { readonly tag: 'denied'; readonly message: string }
    | { readonly tag: 'malformed' };

const parseCallbackParams = (params: URLSearchParams): CallbackParams => {
    const providerError = params.get('error');
    if (providerError) {
        return { tag: 'denied', message: params.get('error_description') || providerError };
    }

    const code = params.get('code');
    const state = params.get('state');
    return code && state ? { tag: 'granted', code, state } : { tag: 'malformed' };
};

type ExchangePhase =
    | { readonly tag: 'exchanging' }
    | { readonly tag: 'failed'; readonly message: string };

const OAuthCallback: React.FC = () => {
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const { state: auth, completeOAuthSignIn } = useAuth();
    const [phase, setPhase] = useState<ExchangePhase>({ tag: 'exchanging' });

    const callback = useMemo(() => parseCallbackParams(searchParams), [searchParams]);

    /*
      The code is single-use: the exchange consumes the stored PKCE verifier, so a second
      attempt with the same code cannot succeed. Keyed here rather than left to the
      effect's identity, because React re-invokes effects on remount (deliberately so in
      development) and the retry would surface as a spurious "missing OAuth state".
    */
    const attempted = useRef<string | null>(null);

    useEffect(() => {
        if (callback.tag !== 'granted') return;
        const attemptKey = `${callback.code}:${callback.state}`;
        if (attempted.current === attemptKey) return;
        attempted.current = attemptKey;

        let cancelled = false;
        completeOAuthSignIn(callback.code, callback.state).then(
            () => {
                if (!cancelled) navigate('/', { replace: true });
            },
            (error: unknown) => {
                if (cancelled) return;
                setPhase({
                    tag: 'failed',
                    message:
                        error instanceof Error ? error.message : 'Failed to complete OAuth login.',
                });
            },
        );
        return () => {
            cancelled = true;
        };
    }, [callback, completeOAuthSignIn, navigate]);

    if (authMode(auth) !== 'oauth') {
        return <Navigate to="/" replace />;
    }

    const failure = ((): string | null => {
        switch (callback.tag) {
            case 'denied':
                return callback.message;
            case 'malformed':
                return 'Missing OAuth callback parameters.';
            case 'granted':
                return phase.tag === 'failed' ? phase.message : null;
            default:
                return assertNever(callback);
        }
    })();

    return (
        <div className="flex min-h-screen flex-col items-center justify-center gap-6 px-6 text-center">
            <Logo asLink={false} />
            {failure ? (
                <div className="max-w-md space-y-3">
                    <h1 className="text-xl font-semibold text-fg">Sign-in failed</h1>
                    <p className="text-sm text-danger">{failure}</p>
                </div>
            ) : (
                <div className="flex items-center gap-2 text-sm text-fg-muted">
                    <Spinner />
                    Completing sign-in…
                </div>
            )}
        </div>
    );
};

export default OAuthCallback;
