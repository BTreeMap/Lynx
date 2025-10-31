import React from 'react';
import { Routes, Route, Navigate } from 'react-router-dom';
import { AuthProvider } from './components/AuthProvider';
import { useAuth } from './hooks/useAuth';
import Login from './components/Login';
import Dashboard from './components/Dashboard';
import UrlDetails from './components/UrlDetails';

const AppContent: React.FC = () => {
  const { authMode, token, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div style={{ 
        textAlign: 'center', 
        padding: '80px 20px',
        color: 'var(--color-text-tertiary)',
        fontSize: '14px'
      }}>
        Loading...
      </div>
    );
  }

  // For auth=none or cloudflare, go directly to dashboard
  // For oauth, show login if no token
  const isAuthenticated = authMode === 'none' || authMode === 'cloudflare' || token;

  return (
    <Routes>
      <Route path="/" element={isAuthenticated ? <Dashboard /> : <Login />} />
      <Route path="/url/:shortCode" element={isAuthenticated ? <UrlDetails /> : <Navigate to="/" replace />} />
      <Route path="*" element={<Navigate to="/" replace />} />
    </Routes>
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

