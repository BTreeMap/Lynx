import React, { useEffect, useState } from 'react';
import { Navigate, useNavigate, useSearchParams } from 'react-router-dom';
import { useAuth } from '../hooks/useAuth';
import { Logo } from './layout/Logo';
import { Spinner } from './ui/Spinner';

const OAuthCallback: React.FC = () => {
  const [searchParams] = useSearchParams();
  const navigate = useNavigate();
  const { authMode, completeOAuthLogin } = useAuth();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const run = async () => {
      const providerError = searchParams.get('error');
      if (providerError) {
        const providerDescription = searchParams.get('error_description');
        setError(providerDescription || providerError);
        return;
      }

      const code = searchParams.get('code');
      const state = searchParams.get('state');
      if (!code || !state) {
        setError('Missing OAuth callback parameters.');
        return;
      }

      try {
        await completeOAuthLogin(code, state);
        navigate('/', { replace: true });
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to complete OAuth login.');
      }
    };

    run();
  }, [completeOAuthLogin, navigate, searchParams]);

  if (authMode !== 'oauth') {
    return <Navigate to="/" replace />;
  }

  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-6 px-6 text-center">
      <Logo asLink={false} />
      {error ? (
        <div className="max-w-md space-y-3">
          <h1 className="text-xl font-semibold text-fg">Sign-in failed</h1>
          <p className="text-sm text-danger">{error}</p>
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
