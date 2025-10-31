import React, { useState, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { apiClient } from '../api';
import type { ShortenedUrl, AnalyticsEntry, AnalyticsAggregate } from '../types';
import { buildShortLink } from '../utils/url';

type AggregateDimension = 'country' | 'region' | 'city' | 'asn' | 'hour' | 'day';

const DIMENSION_LABELS: Record<AggregateDimension, string> = {
  country: 'Country',
  region: 'Region',
  city: 'City',
  asn: 'ASN',
  hour: 'Hour',
  day: 'Day',
};

const UrlDetails: React.FC = () => {
  const { shortCode } = useParams<{ shortCode: string }>();
  const navigate = useNavigate();
  const [url, setUrl] = useState<ShortenedUrl | null>(null);
  const [analytics, setAnalytics] = useState<AnalyticsEntry[]>([]);
  const [aggregateStats, setAggregateStats] = useState<AnalyticsAggregate[]>([]);
  const [selectedDimension, setSelectedDimension] = useState<AggregateDimension>('country');
  const [isLoadingUrl, setIsLoadingUrl] = useState(true);
  const [isLoadingAnalytics, setIsLoadingAnalytics] = useState(true);
  const [isLoadingAggregate, setIsLoadingAggregate] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  // Load URL details
  useEffect(() => {
    const loadUrlData = async () => {
      if (!shortCode) {
        navigate('/');
        return;
      }

      setIsLoadingUrl(true);
      setError(null);

      try {
        const urlData = await apiClient.getUrl(shortCode);
        setUrl(urlData);
      } catch (err: unknown) {
        const error = err as { response?: { data?: { error?: string } } };
        setError(error.response?.data?.error || 'Failed to load URL details');
      } finally {
        setIsLoadingUrl(false);
      }
    };

    loadUrlData();
  }, [shortCode, navigate]);

  // Load analytics data
  useEffect(() => {
    const loadAnalytics = async () => {
      if (!shortCode) return;

      setIsLoadingAnalytics(true);
      try {
        const analyticsData = await apiClient.getAnalytics(shortCode, undefined, undefined, 50);
        setAnalytics(analyticsData.entries);
      } catch (analyticsError) {
        console.warn('Analytics data not available:', analyticsError);
        setAnalytics([]);
      } finally {
        setIsLoadingAnalytics(false);
      }
    };

    loadAnalytics();
  }, [shortCode]);

  // Load aggregate data based on selected dimension
  useEffect(() => {
    const loadAggregate = async () => {
      if (!shortCode) return;

      setIsLoadingAggregate(true);
      try {
        const aggregateData = await apiClient.getAnalyticsAggregate(shortCode, selectedDimension, undefined, undefined, 20);
        setAggregateStats(aggregateData.aggregates);
      } catch (aggregateError) {
        console.warn('Analytics aggregates not available:', aggregateError);
        setAggregateStats([]);
      } finally {
        setIsLoadingAggregate(false);
      }
    };

    loadAggregate();
  }, [shortCode, selectedDimension]);

  const handleCopyLink = async () => {
    if (url) {
      const link = buildShortLink(url.short_code, url.redirect_base_url);
      if (link) {
        try {
          await navigator.clipboard.writeText(link);
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        } catch (err) {
          console.error('Failed to copy:', err);
        }
      }
    }
  };

  const formatDate = (timestamp: number) => {
    return new Date(timestamp * 1000).toLocaleString();
  };

  const formatTimeBucket = (timestamp: number) => {
    const date = new Date(timestamp * 1000);
    return date.toLocaleDateString() + ' ' + date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  };

  const formatDimensionValue = (value: string, dimension: AggregateDimension): string => {
    if (dimension === 'hour' || dimension === 'day') {
      const timestamp = parseInt(value, 10);
      if (!isNaN(timestamp)) {
        const date = new Date(timestamp * 1000);
        if (dimension === 'hour') {
          return date.toLocaleString([], { 
            month: 'short', 
            day: 'numeric', 
            hour: '2-digit', 
            minute: '2-digit' 
          });
        } else {
          return date.toLocaleDateString([], { 
            year: 'numeric', 
            month: 'short', 
            day: 'numeric' 
          });
        }
      }
    }
    return value || 'Unknown';
  };

  // Show error state only if error occurred and we have no URL data
  if (error && !url) {
    return (
      <div style={{ 
        maxWidth: '1200px', 
        margin: '0 auto', 
        padding: '40px 24px'
      }}>
        <div style={{ 
          padding: '14px 16px', 
          marginBottom: '24px', 
          backgroundColor: 'var(--color-error-bg)', 
          color: 'var(--color-error)', 
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--color-error)',
          fontSize: '14px'
        }}>
          {error || 'URL not found'}
        </div>
        <button
          onClick={() => navigate('/')}
          style={{
            padding: '12px 24px',
            backgroundColor: 'var(--color-primary)',
            color: 'var(--color-bg-elevated)',
            border: 'none',
            borderRadius: 'var(--radius-md)',
            fontSize: '14px',
            fontWeight: 500,
            cursor: 'pointer',
            boxShadow: 'var(--shadow-sm)',
          }}
        >
          ← Back to Dashboard
        </button>
      </div>
    );
  }

  const shortLink = url ? buildShortLink(url.short_code, url.redirect_base_url) : null;

  return (
    <div style={{ 
      maxWidth: '1200px', 
      margin: '0 auto', 
      padding: '40px 24px',
      minHeight: '100vh'
    }}>
      {/* Header with Back Button */}
      <div style={{ marginBottom: '32px' }}>
        <button
          onClick={() => navigate('/')}
          style={{
            padding: '8px 16px',
            backgroundColor: 'var(--color-bg-elevated)',
            color: 'var(--color-text-secondary)',
            border: '1px solid var(--color-border)',
            borderRadius: 'var(--radius-md)',
            fontSize: '14px',
            fontWeight: 500,
            cursor: 'pointer',
            boxShadow: 'var(--shadow-sm)',
            marginBottom: '16px'
          }}
        >
          ← Back to Dashboard
        </button>
        <h1 style={{ 
          margin: '0 0 8px 0',
          fontSize: '28px',
          fontWeight: 600,
          color: 'var(--color-text-primary)',
          letterSpacing: '-0.5px'
        }}>
          URL Details & Analytics
        </h1>
        <p style={{ 
          margin: 0, 
          color: 'var(--color-text-tertiary)',
          fontSize: '14px'
        }}>
          View detailed information and analytics for your short link
        </p>
      </div>

      {/* URL Information Card */}
      <div style={{
        backgroundColor: 'var(--color-bg-elevated)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius-lg)',
        boxShadow: 'var(--shadow-sm)',
        padding: '24px',
        marginBottom: '24px'
      }}>
        <h2 style={{
          margin: '0 0 20px 0',
          fontSize: '18px',
          fontWeight: 600,
          color: 'var(--color-text-primary)'
        }}>
          Link Information
        </h2>
        
        {isLoadingUrl ? (
          <div style={{ display: 'grid', gap: '16px' }}>
            <div style={{ 
              height: '60px', 
              backgroundColor: 'var(--color-bg-secondary)', 
              borderRadius: 'var(--radius-md)',
              animation: 'pulse 1.5s ease-in-out infinite'
            }} />
            <div style={{ 
              height: '60px', 
              backgroundColor: 'var(--color-bg-secondary)', 
              borderRadius: 'var(--radius-md)',
              animation: 'pulse 1.5s ease-in-out infinite',
              animationDelay: '0.1s'
            }} />
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '16px' }}>
              {[1, 2, 3, 4].map(i => (
                <div key={i} style={{ 
                  height: '80px', 
                  backgroundColor: 'var(--color-bg-secondary)', 
                  borderRadius: 'var(--radius-md)',
                  animation: 'pulse 1.5s ease-in-out infinite',
                  animationDelay: `${0.1 * i}s`
                }} />
              ))}
            </div>
          </div>
        ) : url ? (
        <div style={{ display: 'grid', gap: '16px' }}>
          {/* Short Link */}
          <div>
            <label style={{
              display: 'block',
              marginBottom: '6px',
              fontSize: '13px',
              fontWeight: 500,
              color: 'var(--color-text-secondary)',
              textTransform: 'uppercase',
              letterSpacing: '0.5px'
            }}>
              Short Link
            </label>
            <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
              <div style={{
                flex: 1,
                padding: '12px 16px',
                backgroundColor: 'var(--color-bg-secondary)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--color-border)',
                wordBreak: 'break-all'
              }}>
                {shortLink ? (
                  <a
                    href={shortLink}
                    target="_blank"
                    rel="noopener noreferrer"
                    style={{
                      color: 'var(--color-text-primary)',
                      fontWeight: 500,
                      fontSize: '14px'
                    }}
                  >
                    {shortLink}
                  </a>
                ) : (
                  <span style={{ color: 'var(--color-text-primary)', fontSize: '14px' }}>
                    {url.short_code}
                  </span>
                )}
              </div>
              <button
                onClick={handleCopyLink}
                style={{
                  padding: '12px 20px',
                  backgroundColor: 'var(--color-bg-elevated)',
                  color: 'var(--color-text-primary)',
                  border: '1px solid var(--color-border)',
                  borderRadius: 'var(--radius-md)',
                  fontSize: '14px',
                  fontWeight: 500,
                  cursor: 'pointer',
                  whiteSpace: 'nowrap',
                  boxShadow: 'var(--shadow-sm)',
                }}
              >
                {copied ? '✓ Copied' : '📋 Copy'}
              </button>
            </div>
          </div>

          {/* Original URL */}
          <div>
            <label style={{
              display: 'block',
              marginBottom: '6px',
              fontSize: '13px',
              fontWeight: 500,
              color: 'var(--color-text-secondary)',
              textTransform: 'uppercase',
              letterSpacing: '0.5px'
            }}>
              Original URL
            </label>
            <div style={{
              padding: '12px 16px',
              backgroundColor: 'var(--color-bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--color-border)',
              wordBreak: 'break-all'
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
            </div>
          </div>

          {/* Stats Grid */}
          <div style={{ 
            display: 'grid', 
            gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', 
            gap: '16px',
            marginTop: '8px'
          }}>
            <div style={{
              padding: '16px',
              backgroundColor: 'var(--color-bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--color-border)'
            }}>
              <div style={{ 
                fontSize: '13px', 
                color: 'var(--color-text-secondary)',
                marginBottom: '4px',
                fontWeight: 500,
                textTransform: 'uppercase',
                letterSpacing: '0.5px'
              }}>
                Total Clicks
              </div>
              <div style={{ 
                fontSize: '24px', 
                fontWeight: 600, 
                color: 'var(--color-text-primary)' 
              }}>
                {url.clicks.toLocaleString()}
              </div>
            </div>

            <div style={{
              padding: '16px',
              backgroundColor: 'var(--color-bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--color-border)'
            }}>
              <div style={{ 
                fontSize: '13px', 
                color: 'var(--color-text-secondary)',
                marginBottom: '4px',
                fontWeight: 500,
                textTransform: 'uppercase',
                letterSpacing: '0.5px'
              }}>
                Status
              </div>
              <div style={{ marginTop: '6px' }}>
                <span style={{
                  padding: '6px 12px',
                  borderRadius: 'var(--radius-sm)',
                  backgroundColor: url.is_active ? 'var(--color-success-bg)' : 'var(--color-error-bg)',
                  color: url.is_active ? 'var(--color-success)' : 'var(--color-error)',
                  fontSize: '13px',
                  fontWeight: 500,
                  border: `1px solid ${url.is_active ? 'var(--color-success)' : 'var(--color-error)'}`
                }}>
                  {url.is_active ? 'Active' : 'Inactive'}
                </span>
              </div>
            </div>

            <div style={{
              padding: '16px',
              backgroundColor: 'var(--color-bg-secondary)',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--color-border)'
            }}>
              <div style={{ 
                fontSize: '13px', 
                color: 'var(--color-text-secondary)',
                marginBottom: '4px',
                fontWeight: 500,
                textTransform: 'uppercase',
                letterSpacing: '0.5px'
              }}>
                Created
              </div>
              <div style={{ 
                fontSize: '14px', 
                fontWeight: 500, 
                color: 'var(--color-text-primary)',
                marginTop: '4px'
              }}>
                {formatDate(url.created_at)}
              </div>
            </div>

            {url.created_by && (
              <div style={{
                padding: '16px',
                backgroundColor: 'var(--color-bg-secondary)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--color-border)'
              }}>
                <div style={{ 
                  fontSize: '13px', 
                  color: 'var(--color-text-secondary)',
                  marginBottom: '4px',
                  fontWeight: 500,
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Created By
                </div>
                <div style={{ 
                  fontSize: '14px', 
                  fontWeight: 500, 
                  color: 'var(--color-text-primary)',
                  marginTop: '4px'
                }}>
                  {url.created_by}
                </div>
              </div>
            )}
          </div>
        </div>
        ) : null}
      </div>

      {/* Aggregate Analytics with Dimension Selector */}
      <div style={{
        backgroundColor: 'var(--color-bg-elevated)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius-lg)',
        boxShadow: 'var(--shadow-sm)',
        padding: '24px',
        marginBottom: '24px'
      }}>
        <div style={{ 
          display: 'flex', 
          justifyContent: 'space-between', 
          alignItems: 'center',
          marginBottom: '20px',
          flexWrap: 'wrap',
          gap: '16px'
        }}>
          <h2 style={{
            margin: 0,
            fontSize: '18px',
            fontWeight: 600,
            color: 'var(--color-text-primary)'
          }}>
            Analytics by Dimension
          </h2>
          
          {/* Dimension Selector */}
          <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
            {(Object.keys(DIMENSION_LABELS) as AggregateDimension[]).map((dimension) => (
              <button
                key={dimension}
                onClick={() => setSelectedDimension(dimension)}
                style={{
                  padding: '8px 16px',
                  backgroundColor: selectedDimension === dimension 
                    ? 'var(--color-primary)' 
                    : 'var(--color-bg-elevated)',
                  color: selectedDimension === dimension 
                    ? 'var(--color-bg-elevated)' 
                    : 'var(--color-text-secondary)',
                  border: `1px solid ${selectedDimension === dimension 
                    ? 'var(--color-primary)' 
                    : 'var(--color-border)'}`,
                  borderRadius: 'var(--radius-md)',
                  fontSize: '13px',
                  fontWeight: 500,
                  cursor: 'pointer',
                  transition: 'all 0.2s ease',
                  boxShadow: selectedDimension === dimension ? 'var(--shadow-sm)' : 'none',
                }}
              >
                {DIMENSION_LABELS[dimension]}
              </button>
            ))}
          </div>
        </div>

        {isLoadingAggregate ? (
          <div style={{ 
            height: '200px', 
            backgroundColor: 'var(--color-bg-secondary)', 
            borderRadius: 'var(--radius-md)',
            animation: 'pulse 1.5s ease-in-out infinite'
          }} />
        ) : aggregateStats.length > 0 ? (
          <div style={{
            overflowX: 'auto',
            border: '1px solid var(--color-border)',
            borderRadius: 'var(--radius-md)'
          }}>
            <table style={{ width: '100%', borderCollapse: 'collapse' }}>
              <thead>
                <tr style={{ 
                  backgroundColor: 'var(--color-bg-secondary)',
                  borderBottom: '1px solid var(--color-border)'
                }}>
                  <th style={{ 
                    padding: '12px 16px', 
                    textAlign: 'left',
                    fontSize: '13px',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.5px'
                  }}>
                    {DIMENSION_LABELS[selectedDimension]}
                  </th>
                  <th style={{ 
                    padding: '12px 16px', 
                    textAlign: 'right',
                    fontSize: '13px',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.5px'
                  }}>
                    Visits
                  </th>
                  <th style={{ 
                    padding: '12px 16px', 
                    textAlign: 'left',
                    fontSize: '13px',
                    fontWeight: 600,
                    color: 'var(--color-text-secondary)',
                    textTransform: 'uppercase',
                    letterSpacing: '0.5px',
                    width: '50%'
                  }}>
                    Distribution
                  </th>
                </tr>
              </thead>
              <tbody>
                {aggregateStats.map((stat, index) => {
                  const totalVisits = aggregateStats.reduce((sum, s) => sum + s.visit_count, 0);
                  const percentage = totalVisits > 0 ? (stat.visit_count / totalVisits) * 100 : 0;
                  return (
                    <tr 
                      key={index}
                      style={{ 
                        borderBottom: index < aggregateStats.length - 1 ? '1px solid var(--color-border-light)' : 'none'
                      }}
                    >
                      <td style={{ 
                        padding: '12px 16px',
                        fontSize: '14px',
                        color: 'var(--color-text-primary)',
                        fontWeight: 500
                      }}>
                        {formatDimensionValue(stat.dimension, selectedDimension)}
                      </td>
                      <td style={{ 
                        padding: '12px 16px',
                        fontSize: '14px',
                        color: 'var(--color-text-primary)',
                        textAlign: 'right',
                        fontWeight: 500
                      }}>
                        {stat.visit_count.toLocaleString()}
                      </td>
                      <td style={{ padding: '12px 16px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                          <div style={{
                            flex: 1,
                            height: '8px',
                            backgroundColor: 'var(--color-bg-secondary)',
                            borderRadius: 'var(--radius-sm)',
                            overflow: 'hidden'
                          }}>
                            <div style={{
                              height: '100%',
                              width: `${percentage}%`,
                              backgroundColor: 'var(--color-primary)',
                              transition: 'width 0.3s ease'
                            }} />
                          </div>
                          <span style={{
                            fontSize: '13px',
                            color: 'var(--color-text-tertiary)',
                            minWidth: '45px',
                            textAlign: 'right'
                          }}>
                            {percentage.toFixed(1)}%
                          </span>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        ) : (
          <div style={{
            padding: '40px 24px',
            textAlign: 'center',
            backgroundColor: 'var(--color-bg-secondary)',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--color-border)'
          }}>
            <p style={{ 
              color: 'var(--color-text-tertiary)',
              fontSize: '14px',
              margin: 0
            }}>
              No {DIMENSION_LABELS[selectedDimension].toLowerCase()} data available yet.
            </p>
          </div>
        )}
      </div>

      {/* Recent Analytics Activity */}
      <div style={{
        backgroundColor: 'var(--color-bg-elevated)',
        border: '1px solid var(--color-border)',
        borderRadius: 'var(--radius-lg)',
        boxShadow: 'var(--shadow-sm)',
        padding: '24px'
      }}>
        <h2 style={{
          margin: '0 0 20px 0',
          fontSize: '18px',
          fontWeight: 600,
          color: 'var(--color-text-primary)'
        }}>
          Recent Activity
        </h2>
        
        {isLoadingAnalytics ? (
          <div style={{ 
            height: '300px', 
            backgroundColor: 'var(--color-bg-secondary)', 
            borderRadius: 'var(--radius-md)',
            animation: 'pulse 1.5s ease-in-out infinite'
          }} />
        ) : analytics.length > 0 ? (
        <div style={{
          overflowX: 'auto',
          border: '1px solid var(--color-border)',
          borderRadius: 'var(--radius-md)'
        }}>
          <table style={{ width: '100%', borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ 
                backgroundColor: 'var(--color-bg-secondary)',
                borderBottom: '1px solid var(--color-border)'
              }}>
                <th style={{ 
                  padding: '12px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Time Period
                </th>
                <th style={{ 
                  padding: '12px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Country
                </th>
                <th style={{ 
                  padding: '12px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Region
                </th>
                <th style={{ 
                  padding: '12px 16px', 
                  textAlign: 'left',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  City
                </th>
                <th style={{ 
                  padding: '12px 16px', 
                  textAlign: 'right',
                  fontSize: '13px',
                  fontWeight: 600,
                  color: 'var(--color-text-secondary)',
                  textTransform: 'uppercase',
                  letterSpacing: '0.5px'
                }}>
                  Visits
                </th>
              </tr>
            </thead>
            <tbody>
              {analytics.slice(0, 20).map((entry, index) => (
                <tr 
                  key={entry.id}
                    style={{ 
                      borderBottom: index < Math.min(analytics.length, 20) - 1 ? '1px solid var(--color-border-light)' : 'none'
                    }}
                  >
                    <td style={{ 
                      padding: '12px 16px',
                      fontSize: '14px',
                      color: 'var(--color-text-primary)'
                    }}>
                      {formatTimeBucket(entry.time_bucket)}
                    </td>
                    <td style={{ 
                      padding: '12px 16px',
                      fontSize: '14px',
                      color: 'var(--color-text-secondary)'
                    }}>
                      {entry.country_code || 'N/A'}
                    </td>
                    <td style={{ 
                      padding: '12px 16px',
                      fontSize: '14px',
                      color: 'var(--color-text-secondary)'
                    }}>
                      {entry.region || 'N/A'}
                    </td>
                    <td style={{ 
                      padding: '12px 16px',
                      fontSize: '14px',
                      color: 'var(--color-text-secondary)'
                    }}>
                      {entry.city || 'N/A'}
                    </td>
                    <td style={{ 
                      padding: '12px 16px',
                      fontSize: '14px',
                      color: 'var(--color-text-primary)',
                      textAlign: 'right',
                      fontWeight: 500
                    }}>
                      {entry.visit_count.toLocaleString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div style={{
            padding: '40px 24px',
            textAlign: 'center',
            backgroundColor: 'var(--color-bg-secondary)',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--color-border)'
          }}>
            <p style={{ 
              color: 'var(--color-text-tertiary)',
              fontSize: '14px',
              margin: 0
            }}>
              No recent activity data available yet. Analytics will appear once your link receives visits.
            </p>
          </div>
        )}
      </div>
    </div>
  );
};

export default UrlDetails;
