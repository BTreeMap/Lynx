import React, { useState } from 'react';
import { ArrowRight, KeyRound, LinkIcon, ShieldCheck, Zap } from 'lucide-react';
import { useAuth } from '../hooks/useAuth';
import { Button } from './ui/Button';
import { ThemeToggle } from './ui/ThemeToggle';
import { Logo } from './layout/Logo';

const highlights = [
  {
    icon: <Zap className="h-4.5 w-4.5" />,
    title: 'Blazing-fast redirects',
    description: 'A Rust-powered engine resolves short links in microseconds.',
  },
  {
    icon: <LinkIcon className="h-4.5 w-4.5" />,
    title: 'Custom short codes',
    description: 'Brand every link with a memorable, human-friendly slug.',
  },
  {
    icon: <ShieldCheck className="h-4.5 w-4.5" />,
    title: 'Privacy-first analytics',
    description: 'Understand reach with aggregated geo and network insights.',
  },
];

const Login: React.FC = () => {
  const [isSigningIn, setIsSigningIn] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { startOAuthLogin } = useAuth();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setIsSigningIn(true);

    try {
      await startOAuthLogin();
    } catch (err) {
      setIsSigningIn(false);
      setError(err instanceof Error ? err.message : 'Failed to start OAuth login.');
    }
  };

  return (
    <div className="grid min-h-screen lg:grid-cols-2">
      {/* Brand / marketing panel */}
      <aside className="relative hidden flex-col justify-between overflow-hidden bg-baltic-blue-950 p-10 text-baltic-blue-50 lg:flex">
        <div
          className="pointer-events-none absolute inset-0 opacity-70"
          style={{
            backgroundImage:
              'radial-gradient(60% 50% at 20% 10%, rgba(110,142,171,0.25), transparent 60%), radial-gradient(50% 50% at 90% 90%, rgba(129,210,199,0.16), transparent 60%)',
          }}
          aria-hidden
        />
        <div className="relative">
          <Logo asLink={false} wordmarkClassName="text-white" />
        </div>

        <div className="relative space-y-8">
          <div className="space-y-3">
            <h1 className="text-4xl font-bold leading-tight tracking-tight text-white">
              Short links,
              <br />
              serious performance.
            </h1>
            <p className="max-w-md text-baltic-blue-200">
              Lynx turns long, unwieldy URLs into fast, trackable short links —
              backed by a high-performance Rust core.
            </p>
          </div>

          <ul className="space-y-4">
            {highlights.map((item) => (
              <li key={item.title} className="flex items-start gap-3">
                <span className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-white/10 text-baltic-blue-100 backdrop-blur">
                  {item.icon}
                </span>
                <div>
                  <p className="font-semibold text-white">{item.title}</p>
                  <p className="text-sm text-baltic-blue-200">{item.description}</p>
                </div>
              </li>
            ))}
          </ul>
        </div>

        <p className="relative text-xs text-baltic-blue-300">
          © {new Date().getFullYear()} Lynx. Built for speed.
        </p>
      </aside>

      {/* Auth panel */}
      <main className="flex flex-col px-4 py-6 sm:px-10 sm:py-8">
        <div className="flex items-center justify-between lg:justify-end">
          <Logo className="lg:hidden" asLink={false} />
          <ThemeToggle />
        </div>

        <div className="flex flex-1 items-center justify-center">
          <div className="w-full max-w-md py-8 sm:py-10">
            <div className="mb-6 flex flex-col items-center text-center sm:mb-8">
              <span className="mb-3.5 flex h-10 w-10 items-center justify-center rounded-2xl bg-primary-soft text-primary-soft-fg sm:mb-4 sm:h-12 sm:w-12">
                <KeyRound className="h-6 w-6" />
              </span>
              <h2 className="text-xl font-bold tracking-tight text-fg sm:text-2xl">Welcome back</h2>
              <p className="mt-1 text-sm text-fg-muted sm:mt-1.5">
                Sign in with your OAuth provider to access your dashboard.
              </p>
            </div>

            <form onSubmit={handleSubmit} className="space-y-4 sm:space-y-5">
              <Button
                type="submit"
                size="lg"
                fullWidth
                isLoading={isSigningIn}
                rightIcon={<ArrowRight className="h-4 w-4" />}
              >
                Sign in with OAuth
              </Button>

              {error && (
                <p className="rounded-xl border border-danger/40 bg-danger/5 px-3 py-2 text-sm text-danger">
                  {error}
                </p>
              )}
            </form>

            <div className="mt-6 rounded-2xl border border-border bg-surface-2/50 p-4 sm:mt-8 sm:p-5">
              <h3 className="text-xs font-semibold uppercase tracking-wide text-fg-subtle">
                Authentication flow
              </h3>
              <ol className="mt-3 space-y-2 text-sm text-fg-muted">
                <li className="flex gap-2.5">
                  <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary-soft text-xs font-semibold text-primary-soft-fg">
                    1
                  </span>
                  You are redirected to your OAuth provider to sign in.
                </li>
                <li className="flex gap-2.5">
                  <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary-soft text-xs font-semibold text-primary-soft-fg">
                    2
                  </span>
                  Lynx completes PKCE code exchange in your browser.
                </li>
                <li className="flex gap-2.5">
                  <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary-soft text-xs font-semibold text-primary-soft-fg">
                    3
                  </span>
                  A bearer token is stored locally and sent in HTTP headers.
                </li>
              </ol>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
};

export default Login;
