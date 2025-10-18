import React, { useState } from 'react';
import { apiClient } from '../api';
import type { CreateUrlRequest } from '../types';
import { buildShortLink } from '../utils/url';

interface CreateUrlFormProps {
  onUrlCreated: () => void;
}

const CreateUrlForm: React.FC<CreateUrlFormProps> = ({ onUrlCreated }) => {
  const [url, setUrl] = useState('');
  const [customCode, setCustomCode] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [successCode, setSuccessCode] = useState<string | null>(null);
  const [successLink, setSuccessLink] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSubmitting(true);
    setError(null);
    setSuccessCode(null);
    setSuccessLink(null);

    try {
      const request: CreateUrlRequest = {
        url,
        custom_code: customCode || undefined,
      };
      const result = await apiClient.createUrl(request);
      const fullLink = buildShortLink(result.short_code, result.redirect_base_url);
      setSuccessCode(result.short_code);
      setSuccessLink(fullLink);
      setUrl('');
      setCustomCode('');
      onUrlCreated();
    } catch (err: any) {
      setError(err.response?.data?.error || 'Failed to create URL');
    } finally {
      setIsSubmitting(false);
    }
  };

  return (
    <div style={{ 
      marginBottom: '40px', 
      padding: '24px', 
      backgroundColor: 'var(--color-bg-elevated)',
      border: '1px solid var(--color-border)', 
      borderRadius: 'var(--radius-lg)',
      boxShadow: 'var(--shadow-sm)'
    }}>
      <h2 style={{ 
        marginTop: 0,
        marginBottom: '20px',
        fontSize: '18px',
        fontWeight: 600,
        color: 'var(--color-text-primary)'
      }}>
        Create Short URL
      </h2>
      <form onSubmit={handleSubmit}>
        <div style={{ marginBottom: '16px' }}>
          <label htmlFor="url" style={{ 
            display: 'block', 
            marginBottom: '8px',
            fontSize: '14px',
            fontWeight: 500,
            color: 'var(--color-text-primary)'
          }}>
            Original URL *
          </label>
          <input
            type="url"
            id="url"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://example.com/very/long/url"
            style={{
              width: '100%',
              padding: '10px 12px',
              fontSize: '14px',
              border: '1px solid var(--color-border)',
              borderRadius: 'var(--radius-md)',
              backgroundColor: 'var(--color-bg)',
              color: 'var(--color-text-primary)',
            }}
            required
          />
        </div>
        <div style={{ marginBottom: '20px' }}>
          <label htmlFor="customCode" style={{ 
            display: 'block', 
            marginBottom: '8px',
            fontSize: '14px',
            fontWeight: 500,
            color: 'var(--color-text-primary)'
          }}>
            Custom Code (optional)
          </label>
          <input
            type="text"
            id="customCode"
            value={customCode}
            onChange={(e) => setCustomCode(e.target.value)}
            placeholder="my-custom-code"
            maxLength={20}
            style={{
              width: '100%',
              padding: '10px 12px',
              fontSize: '14px',
              border: '1px solid var(--color-border)',
              borderRadius: 'var(--radius-md)',
              backgroundColor: 'var(--color-bg)',
              color: 'var(--color-text-primary)',
            }}
          />
          <small style={{ 
            color: 'var(--color-text-tertiary)',
            fontSize: '13px'
          }}>
            Leave empty for auto-generated code
          </small>
        </div>
        {error && (
          <div style={{ 
            padding: '12px 14px', 
            marginBottom: '16px', 
            backgroundColor: 'var(--color-error-bg)', 
            color: 'var(--color-error)', 
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--color-error)',
            fontSize: '14px'
          }}>
            {error}
          </div>
        )}
        {successCode && (
          <div style={{ 
            padding: '12px 14px', 
            marginBottom: '16px', 
            backgroundColor: 'var(--color-success-bg)', 
            color: 'var(--color-success)', 
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--color-success)',
            fontSize: '14px'
          }}>
            Created short URL:{' '}
            {successLink ? (
              <a
                href={successLink}
                target="_blank"
                rel="noopener noreferrer"
                style={{ 
                  color: 'var(--color-success)', 
                  fontWeight: 600,
                  textDecoration: 'underline'
                }}
              >
                {successLink}
              </a>
            ) : (
              <span style={{ fontWeight: 600 }}>{successCode}</span>
            )}
          </div>
        )}
        <button
          type="submit"
          disabled={isSubmitting}
          style={{
            padding: '12px 24px',
            backgroundColor: 'var(--color-primary)',
            color: 'var(--color-bg-elevated)',
            border: 'none',
            borderRadius: 'var(--radius-md)',
            fontSize: '14px',
            fontWeight: 500,
            cursor: isSubmitting ? 'not-allowed' : 'pointer',
            boxShadow: 'var(--shadow-sm)',
          }}
        >
          {isSubmitting ? 'Creating...' : 'Create Short URL'}
        </button>
      </form>
    </div>
  );
};

export default CreateUrlForm;
