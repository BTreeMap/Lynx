import React, { useState } from 'react';
import { Link } from 'react-router-dom';
import { apiClient } from '../api';
import type { ShortenedUrl } from '../types';
import { buildShortLink } from '../utils/url';

interface UrlListProps {
  urls: ShortenedUrl[];
  isAdmin: boolean;
  onUrlsChanged: () => void;
}

const UrlList: React.FC<UrlListProps> = ({ urls, isAdmin, onUrlsChanged }) => {
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copiedCode, setCopiedCode] = useState<string | null>(null);

  const handleDeactivate = async (code: string) => {
    if (!confirm(`Are you sure you want to deactivate the URL: ${code}?`)) {
      return;
    }
    setActionInProgress(code);
    setError(null);
    try {
      await apiClient.deactivateUrl(code);
      onUrlsChanged();
    } catch (err: unknown) {
      const error = err as { response?: { data?: { error?: string } } };
      setError(error.response?.data?.error || 'Failed to deactivate URL');
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
    } catch (err: unknown) {
      const error = err as { response?: { data?: { error?: string } } };
      setError(error.response?.data?.error || 'Failed to reactivate URL');
    } finally {
      setActionInProgress(null);
    }
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const buildLinkForItem = (item: ShortenedUrl) => buildShortLink(item.short_code, item.redirect_base_url);

  const handleCopyLink = async (shortCode: string) => {
    const url = urls.find(u => u.short_code === shortCode);
    if (url) {
      const link = buildLinkForItem(url);
      if (link) {
        try {
          await navigator.clipboard.writeText(link);
          setCopiedCode(shortCode);
          setTimeout(() => setCopiedCode(null), 2000);
        } catch (err) {
          console.error('Failed to copy:', err);
        }
      }
    }
  };

  return (
    <div>
      <h2 style={{ 
        marginBottom: '20px',
        fontSize: '18px',
        fontWeight: 600,
        color: 'var(--color-text-primary)'
      }}>
        Your URLs
      </h2>
      {error && (
        <div style={{ 
          padding: '12px 14px', 
          marginBottom: '20px', 
          backgroundColor: 'var(--color-error-bg)', 
          color: 'var(--color-error)', 
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--color-error)',
          fontSize: '14px'
        }}>
          {error}
        </div>
      )}
      {urls.length === 0 ? (
        <p style={{ 
          color: 'var(--color-text-tertiary)',
          fontSize: '14px',
          padding: '40px 0',
          textAlign: 'center'
        }}>
          No URLs found. Create your first short URL above!
        </p>
      ) : (
        <div style={{ 
          overflowX: 'auto',
          backgroundColor: 'var(--color-bg-elevated)',
          border: '1px solid var(--color-border)',
          borderRadius: 'var(--radius-lg)',
          boxShadow: 'var(--shadow-sm)'
        }}>
          <table style={{ 
            width: '100%', 
            borderCollapse: 'collapse'
          }}>
            <thead>
              <tr style={{ 
                backgroundColor: 'var(--color-bg-secondary)',
                borderBottom: '1px solid var(--color-border)'
              }}>
                <th style={{ 
                  padding: '14px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Short Code
                </th>
                <th style={{ 
                  padding: '14px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px',
                  width: '100px'
                }}>
                  Copy
                </th>
                <th style={{ 
                  padding: '14px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Original URL
                </th>
                <th style={{ 
                  padding: '14px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Clicks
                </th>
                <th style={{ 
                  padding: '14px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Status
                </th>
                <th style={{ 
                  padding: '14px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Created
                </th>
                {isAdmin && (
                  <th style={{ 
                    padding: '14px 16px', 
                    textAlign: 'left',
                    fontSize: '13px',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.5px'
                  }}>
                    Created By
                  </th>
                )}
                {isAdmin && (
                  <th style={{ 
                    padding: '14px 16px', 
                    textAlign: 'left',
                    fontSize: '13px',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.5px'
                  }}>
                    Actions
                  </th>
                )}
              </tr>
            </thead>
            <tbody>
              {urls.map((url) => (
                  <tr 
                    key={url.id} 
                    style={{ 
                      borderBottom: '1px solid var(--color-border-light)',
                      transition: 'background-color 0.15s ease'
                    }}
                    onMouseEnter={(e) => e.currentTarget.style.backgroundColor = 'var(--color-bg)'}
                    onMouseLeave={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                  >
                    <td style={{ padding: '14px 16px' }}>
                      <Link
                        to={`/url/${url.short_code}`}
                        style={{ 
                          color: 'var(--color-text-primary)',
                          fontWeight: 500,
                          fontSize: '14px'
                        }}
                      >
                        {url.short_code}
                      </Link>
                    </td>
                    <td style={{ padding: '14px 16px' }}>
                      <button
                        onClick={() => handleCopyLink(url.short_code)}
                        style={{
                          padding: '6px 12px',
                          backgroundColor: 'var(--color-bg-elevated)',
                          color: 'var(--color-text-primary)',
                          border: '1px solid var(--color-border)',
                          borderRadius: 'var(--radius-sm)',
                          fontSize: '12px',
                          fontWeight: 500,
                          cursor: 'pointer',
                          whiteSpace: 'nowrap'
                        }}
                        title="Copy link to clipboard"
                      >
                        {copiedCode === url.short_code ? '✓ Copied' : '📋 Copy'}
                      </button>
                    </td>
                    <td style={{ 
                      padding: '14px 16px', 
                      maxWidth: '300px', 
                      overflow: 'hidden', 
                      textOverflow: 'ellipsis', 
                      whiteSpace: 'nowrap' 
                    }}>
                      <a 
                        href={url.original_url} 
                        target="_blank" 
                        rel="noopener noreferrer" 
                        style={{ 
                          color: 'var(--color-text-secondary)',
                          fontSize: '14px'
                        }}
                      >
                        {url.original_url}
                      </a>
                    </td>
                    <td style={{ 
                      padding: '14px 16px',
                      fontSize: '14px',
                      color: 'var(--color-text-primary)'
                    }}>
                      {url.clicks}
                    </td>
                    <td style={{ padding: '14px 16px' }}>
                      <span
                        style={{
                          padding: '4px 10px',
                          borderRadius: 'var(--radius-sm)',
                          backgroundColor: url.is_active ? 'var(--color-success-bg)' : 'var(--color-error-bg)',
                          color: url.is_active ? 'var(--color-success)' : 'var(--color-error)',
                          fontSize: '12px',
                          fontWeight: 500,
                          border: `1px solid ${url.is_active ? 'var(--color-success)' : 'var(--color-error)'}`
                        }}
                      >
                        {url.is_active ? 'Active' : 'Inactive'}
                      </span>
                    </td>
                    <td style={{ 
                      padding: '14px 16px', 
                      fontSize: '13px', 
                      color: 'var(--color-text-tertiary)' 
                    }}>
                      {formatDate(url.created_at)}
                    </td>
                    {isAdmin && (
                      <td style={{ 
                        padding: '14px 16px', 
                        fontSize: '13px', 
                        color: 'var(--color-text-tertiary)' 
                      }}>
                        {url.created_by || 'N/A'}
                      </td>
                    )}
                    {isAdmin && (
                      <td style={{ padding: '14px 16px' }}>
                        {url.is_active ? (
                          <button
                            onClick={() => handleDeactivate(url.short_code)}
                            disabled={actionInProgress === url.short_code}
                            style={{
                              padding: '6px 14px',
                              backgroundColor: 'var(--color-bg-elevated)',
                              color: 'var(--color-error)',
                              border: '1px solid var(--color-error)',
                              borderRadius: 'var(--radius-sm)',
                              fontSize: '12px',
                              fontWeight: 500,
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
                              padding: '6px 14px',
                              backgroundColor: 'var(--color-bg-elevated)',
                              color: 'var(--color-success)',
                              border: '1px solid var(--color-success)',
                              borderRadius: 'var(--radius-sm)',
                              fontSize: '12px',
                              fontWeight: 500,
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
