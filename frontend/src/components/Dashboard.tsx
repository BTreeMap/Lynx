import React, { useState, useEffect } from 'react';
import { useAuth } from '../AuthContext';
import { apiClient } from '../api';
import CreateUrlForm from './CreateUrlForm';
import UrlList from './UrlList';
import type { ShortenedUrl } from '../types';

const Dashboard: React.FC = () => {
  const { userInfo } = useAuth();
  const [urls, setUrls] = useState<ShortenedUrl[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
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

  const exportToJson = async () => {
    setIsExporting(true);
    setError(null);
    try {
      // Fetch all URLs using pagination
      const allUrls: ShortenedUrl[] = [];
      let cursor: string | null = null;
      let hasMoreData = true;

      while (hasMoreData) {
        const data = await apiClient.listUrls(50, cursor || undefined);
        allUrls.push(...data.urls);
        cursor = data.next_cursor || null;
        hasMoreData = data.has_more;
      }

      // Create JSON blob and download
      const jsonStr = JSON.stringify(allUrls, null, 2);
      const blob = new Blob([jsonStr], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = `lynx-urls-export-${new Date().toISOString().split('T')[0]}.json`;
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      URL.revokeObjectURL(url);
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to export URLs');
    } finally {
      setIsExporting(false);
    }
  };

  useEffect(() => {
    loadUrls(true);
  }, []);

  return (
    <div style={{ 
      maxWidth: '1200px', 
      margin: '0 auto', 
      padding: '40px 24px',
      minHeight: '100vh'
    }}>
      {/* Header */}
      <div style={{ 
        display: 'flex', 
        justifyContent: 'space-between', 
        alignItems: 'center', 
        marginBottom: '40px',
        paddingBottom: '24px',
        borderBottom: '1px solid var(--color-border)'
      }}>
        <div>
          <h1 style={{ 
            margin: 0,
            fontSize: '28px',
            fontWeight: 600,
            color: 'var(--color-text-primary)',
            letterSpacing: '-0.5px'
          }}>
            Lynx
          </h1>
          {userInfo && (
            <p style={{ 
              margin: '8px 0 0 0', 
              color: 'var(--color-text-secondary)',
              fontSize: '14px'
            }}>
              {userInfo.user_id || 'Unknown'}
              {userInfo.is_admin && (
                <span style={{ 
                  marginLeft: '12px', 
                  padding: '3px 10px', 
                  backgroundColor: 'var(--color-text-primary)',
                  color: 'var(--color-bg-elevated)', 
                  borderRadius: 'var(--radius-sm)', 
                  fontSize: '11px',
                  fontWeight: 600,
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Admin
                </span>
              )}
            </p>
          )}
        </div>
        <div style={{ display: 'flex', gap: '12px' }}>
          <button
            onClick={exportToJson}
            disabled={isExporting}
            style={{
              padding: '10px 20px',
              backgroundColor: 'var(--color-bg-elevated)',
              color: 'var(--color-text-primary)',
              border: '1px solid var(--color-border)',
              borderRadius: 'var(--radius-md)',
              fontSize: '14px',
              fontWeight: 500,
              cursor: isExporting ? 'not-allowed' : 'pointer',
              boxShadow: 'var(--shadow-sm)',
            }}
            title="Export all URLs to JSON"
          >
            {isExporting ? 'Exporting...' : '↓ Export JSON'}
          </button>
        </div>
      </div>

      <CreateUrlForm onUrlCreated={() => loadUrls(true)} />

      {error && (
        <div style={{ 
          padding: '14px 16px', 
          marginBottom: '24px', 
          backgroundColor: 'var(--color-error-bg)', 
          color: 'var(--color-error)', 
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--color-error)',
          fontSize: '14px'
        }}>
          {error}
        </div>
      )}

      {isLoading ? (
        <div style={{ 
          textAlign: 'center', 
          padding: '60px 20px', 
          color: 'var(--color-text-tertiary)',
          fontSize: '14px'
        }}>
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
            <div style={{ textAlign: 'center', marginTop: '32px' }}>
              <button
                onClick={loadMoreUrls}
                disabled={isLoadingMore}
                style={{
                  padding: '12px 32px',
                  backgroundColor: 'var(--color-bg-elevated)',
                  color: 'var(--color-text-primary)',
                  border: '1px solid var(--color-border)',
                  borderRadius: 'var(--radius-md)',
                  fontSize: '14px',
                  fontWeight: 500,
                  cursor: isLoadingMore ? 'not-allowed' : 'pointer',
                  boxShadow: 'var(--shadow-sm)',
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
