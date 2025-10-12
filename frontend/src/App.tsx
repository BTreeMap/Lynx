import React from 'react';
import { AuthProvider, useAuth } from './AuthContext';
import Login from './components/Login';
import Dashboard from './components/Dashboard';

const AppContent: React.FC = () => {
  const { authMode, token, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div style={{ textAlign: 'center', padding: '50px' }}>
        Loading...
      </div>
    );
  }

  // For auth=none or cloudflare, go directly to dashboard
  // For oauth, show login if no token
  if (authMode === 'none' || authMode === 'cloudflare') {
    return <Dashboard />;
  }

  return token ? <Dashboard /> : <Login />;
};

const App: React.FC = () => {
  return (
    <AuthProvider>
      <AppContent />
    </AuthProvider>
  );
};

export default App;

