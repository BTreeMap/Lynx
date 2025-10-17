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
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [nextCursor, setNextCursor] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);

  const loadUrls = async (reset = true) => {
    if (reset) {
      setIsLoading(true);
      setUrls([]);
      setNextCursor(null);
    }
    setError(null);
    try {
      const data = await apiClient.listUrls(50);
      if (reset) {
        setUrls(data.urls);
      } else {
        setUrls(prev => [...prev, ...data.urls]);
      }
      setNextCursor(data.next_cursor || null);
      setHasMore(data.has_more);
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to load URLs');
    } finally {
      setIsLoading(false);
    }
  };

  const loadMoreUrls = async () => {
    if (!nextCursor || isLoadingMore) return;
    
    setIsLoadingMore(true);
    setError(null);
    try {
      const data = await apiClient.listUrls(50, nextCursor);
      setUrls(prev => [...prev, ...data.urls]);
      setNextCursor(data.next_cursor || null);
      setHasMore(data.has_more);
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to load more URLs');
    } finally {
      setIsLoadingMore(false);
    }
  };

  useEffect(() => {
    loadUrls(true);
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

      <CreateUrlForm onUrlCreated={() => loadUrls(true)} />

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
        <>
          <UrlList 
            urls={urls} 
            isAdmin={userInfo?.is_admin || false} 
            onUrlsChanged={() => loadUrls(true)}
          />
          {hasMore && (
            <div style={{ textAlign: 'center', marginTop: '20px' }}>
              <button
                onClick={loadMoreUrls}
                disabled={isLoadingMore}
                style={{
                  padding: '10px 20px',
                  backgroundColor: isLoadingMore ? '#ccc' : '#007bff',
                  color: 'white',
                  border: 'none',
                  borderRadius: '4px',
                  fontSize: '14px',
                  cursor: isLoadingMore ? 'not-allowed' : 'pointer',
                }}
              >
                {isLoadingMore ? 'Loading...' : 'Load More'}
              </button>
            </div>
          )}
        </>
      )}
    </div>
  );
};

export default Dashboard;
