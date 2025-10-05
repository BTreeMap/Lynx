import React, { useState, useEffect } from 'react';
import { useAuth } from '../AuthContext';
import { apiClient } from '../api';
import CreateUrlForm from './CreateUrlForm';
import UrlList from './UrlList';
import type { ShortenedUrl } from '../types';

const Dashboard: React.FC = () => {
  const { userInfo, logout } = useAuth();
  const [urls, setUrls] = useState<ShortenedUrl[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadUrls = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await apiClient.listUrls();
      setUrls(data);
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to load URLs');
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadUrls();
  }, []);

  return (
    <div style={{ maxWidth: '1200px', margin: '0 auto', padding: '20px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: '30px' }}>
        <div>
          <h1 style={{ margin: 0 }}>Lynx URL Shortener</h1>
          {userInfo && (
            <p style={{ margin: '5px 0 0 0', color: '#666' }}>
              User: {userInfo.user_id || 'Unknown'}
              {userInfo.is_admin && <span style={{ 
                marginLeft: '10px', 
                padding: '2px 8px', 
                backgroundColor: '#ffc107', 
                color: '#000', 
                borderRadius: '4px', 
                fontSize: '12px' 
              }}>ADMIN</span>}
            </p>
          )}
        </div>
        <button
          onClick={logout}
          style={{
            padding: '8px 16px',
            backgroundColor: '#6c757d',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            fontSize: '14px',
            cursor: 'pointer',
          }}
        >
          Logout
        </button>
      </div>

      <CreateUrlForm onUrlCreated={loadUrls} />

      {error && (
        <div style={{ padding: '10px', marginBottom: '15px', backgroundColor: '#f8d7da', color: '#721c24', borderRadius: '4px' }}>
          {error}
        </div>
      )}

      {isLoading ? (
        <div style={{ textAlign: 'center', padding: '20px', color: '#666' }}>
          Loading...
        </div>
      ) : (
        <UrlList 
          urls={urls} 
          isAdmin={userInfo?.is_admin || false} 
          onUrlsChanged={loadUrls}
        />
      )}
    </div>
  );
};

export default Dashboard;
