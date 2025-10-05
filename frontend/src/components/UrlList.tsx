import React, { useState } from 'react';
import { apiClient } from '../api';
import type { ShortenedUrl } from '../types';

interface UrlListProps {
  urls: ShortenedUrl[];
  isAdmin: boolean;
  onUrlsChanged: () => void;
}

const UrlList: React.FC<UrlListProps> = ({ urls, isAdmin, onUrlsChanged }) => {
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleDeactivate = async (code: string) => {
    if (!confirm(`Are you sure you want to deactivate the URL: ${code}?`)) {
      return;
    }
    setActionInProgress(code);
    setError(null);
    try {
      await apiClient.deactivateUrl(code);
      onUrlsChanged();
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to deactivate URL');
    } finally {
      setActionInProgress(null);
    }
  };

  const handleReactivate = async (code: string) => {
    if (!confirm(`Are you sure you want to reactivate the URL: ${code}?`)) {
      return;
    }
    setActionInProgress(code);
    setError(null);
    try {
      await apiClient.reactivateUrl(code);
      onUrlsChanged();
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to reactivate URL');
    } finally {
      setActionInProgress(null);
    }
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const getRedirectUrl = () => {
    const redirectHost = import.meta.env.VITE_REDIRECT_URL || 'http://localhost:3000';
    return redirectHost;
  };

  return (
    <div>
      <h2>Your URLs</h2>
      {error && (
        <div style={{ padding: '10px', marginBottom: '15px', backgroundColor: '#f8d7da', color: '#721c24', borderRadius: '4px' }}>
          {error}
        </div>
      )}
      {urls.length === 0 ? (
        <p style={{ color: '#666' }}>No URLs found. Create your first short URL above!</p>
      ) : (
        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ backgroundColor: '#f8f9fa' }}>
                <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Short Code</th>
                <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Original URL</th>
                <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Clicks</th>
                <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Status</th>
                <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Created</th>
                {isAdmin && <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Created By</th>}
                {isAdmin && <th style={{ padding: '12px', textAlign: 'left', borderBottom: '2px solid #dee2e6' }}>Actions</th>}
              </tr>
            </thead>
            <tbody>
              {urls.map((url) => (
                <tr key={url.id} style={{ borderBottom: '1px solid #dee2e6' }}>
                  <td style={{ padding: '12px' }}>
                    <a 
                      href={`${getRedirectUrl()}/${url.short_code}`} 
                      target="_blank" 
                      rel="noopener noreferrer"
                      style={{ color: '#007bff', textDecoration: 'none' }}
                    >
                      {url.short_code}
                    </a>
                  </td>
                  <td style={{ padding: '12px', maxWidth: '300px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                    <a href={url.original_url} target="_blank" rel="noopener noreferrer" style={{ color: '#666', textDecoration: 'none' }}>
                      {url.original_url}
                    </a>
                  </td>
                  <td style={{ padding: '12px' }}>{url.clicks}</td>
                  <td style={{ padding: '12px' }}>
                    <span style={{
                      padding: '4px 8px',
                      borderRadius: '4px',
                      backgroundColor: url.is_active ? '#d4edda' : '#f8d7da',
                      color: url.is_active ? '#155724' : '#721c24',
                      fontSize: '12px',
                    }}>
                      {url.is_active ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td style={{ padding: '12px', fontSize: '13px', color: '#666' }}>{formatDate(url.created_at)}</td>
                  {isAdmin && <td style={{ padding: '12px', fontSize: '13px', color: '#666' }}>{url.created_by || 'N/A'}</td>}
                  {isAdmin && (
                    <td style={{ padding: '12px' }}>
                      {url.is_active ? (
                        <button
                          onClick={() => handleDeactivate(url.short_code)}
                          disabled={actionInProgress === url.short_code}
                          style={{
                            padding: '6px 12px',
                            backgroundColor: actionInProgress === url.short_code ? '#ccc' : '#dc3545',
                            color: 'white',
                            border: 'none',
                            borderRadius: '4px',
                            fontSize: '12px',
                            cursor: actionInProgress === url.short_code ? 'not-allowed' : 'pointer',
                          }}
                        >
                          Deactivate
                        </button>
                      ) : (
                        <button
                          onClick={() => handleReactivate(url.short_code)}
                          disabled={actionInProgress === url.short_code}
                          style={{
                            padding: '6px 12px',
                            backgroundColor: actionInProgress === url.short_code ? '#ccc' : '#28a745',
                            color: 'white',
                            border: 'none',
                            borderRadius: '4px',
                            fontSize: '12px',
                            cursor: actionInProgress === url.short_code ? 'not-allowed' : 'pointer',
                          }}
                        >
                          Reactivate
                        </button>
                      )}
                    </td>
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};

export default UrlList;
