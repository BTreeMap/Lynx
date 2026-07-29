import React, { Suspense, lazy } from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { isAuthorized, isBooting } from './auth/model';
import { AuthProvider } from './components/AuthProvider';
import { useAuth } from './hooks/useAuth';
import Login from './components/Login';
import OAuthCallback from './components/OAuthCallback';
import { Logo } from './components/layout/Logo';
import { Spinner } from './components/ui/Spinner';

// Split out of the entry chunk: the dashboard pulls in the virtualiser and the details
// route pulls in Recharts, neither of which the sign-in screen needs.
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
    const { state } = useAuth();

    if (isBooting(state)) {
        return <Splash />;
    }

    /*
      One selector, not a disjunction repeated per route. "May the dashboard be shown"
      is a property of the session union — pass-through modes are authorized without a
      token, OAuth is authorized only with one — and deriving it here means the two
      protected routes cannot disagree about it.
    */
    const authorized = isAuthorized(state);

    return (
        <Suspense fallback={<Splash message="Loading…" />}>
            <Routes>
                <Route path="/" element={authorized ? <Dashboard /> : <Login />} />
                <Route path="/auth/callback" element={<OAuthCallback />} />
                <Route
                    path="/url/:shortCode"
                    element={authorized ? <UrlDetails /> : <Navigate to="/" replace />}
                />
                <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
        </Suspense>
    );
};

const App: React.FC = () => (
    <AuthProvider>
        <AppContent />
    </AuthProvider>
);

export default App;
