import React, { Suspense, lazy } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { AuthProvider } from './components/AuthProvider';
import { useAuth } from './hooks/useAuth';
import Login from './components/Login';
import OAuthCallback from './components/OAuthCallback';
import { Logo } from './components/layout/Logo';
import { Spinner } from './components/ui/Spinner';

const Dashboard = lazy(() => import('./components/Dashboard'));
const UrlDetails = lazy(() => import('./components/UrlDetails'));

const Splash: React.FC<{ message?: string }> = ({ message = 'Loading your workspace…' }) => (
  <div className="flex min-h-screen flex-col items-center justify-center gap-6">
    <Logo asLink={false} />
    <div className="flex items-center gap-2 text-sm text-fg-muted">
      <Spinner />
      {message}
    </div>
  </div>
);

const AppContent: React.FC = () => {
  const { authMode, token, isLoading } = useAuth();

  if (isLoading) {
    return <Splash />;
  }

  // For auth=none or cloudflare, go directly to dashboard
  // For oauth, show login if no token
  const isAuthenticated = authMode === 'none' || authMode === 'cloudflare' || token;

  return (
    <Suspense fallback={<Splash message="Loading…" />}>
      <Routes>
        <Route path="/" element={isAuthenticated ? <Dashboard /> : <Login />} />
        <Route path="/auth/callback" element={<OAuthCallback />} />
        <Route
          path="/url/:shortCode"
          element={isAuthenticated ? <UrlDetails /> : <Navigate to="/" replace />}
        />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Suspense>
  );
};

const App: React.FC = () => {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
};

export default App;

