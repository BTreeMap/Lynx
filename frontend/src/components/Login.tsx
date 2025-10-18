import React, { useState } from 'react';
import { useAuth } from '../AuthContext';

const Login: React.FC = () => {
  const [token, setToken] = useState('');
  const { login } = useAuth();

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (token.trim()) {
      login(token.trim());
    }
  };

  return (
    <div style={{ 
      maxWidth: '440px', 
      margin: '80px auto', 
      padding: '32px',
      backgroundColor: 'var(--color-bg-elevated)',
      border: '1px solid var(--color-border)',
      borderRadius: 'var(--radius-lg)',
      boxShadow: 'var(--shadow-md)'
    }}>
      <h1 style={{ 
        fontSize: '24px',
        fontWeight: 600,
        marginBottom: '8px',
        color: 'var(--color-text-primary)',
        letterSpacing: '-0.5px'
      }}>
        Lynx
      </h1>
      <p style={{ 
        marginBottom: '28px',
        color: 'var(--color-text-secondary)',
        fontSize: '14px'
      }}>
        Enter your OAuth 2.0 bearer token to continue.
      </p>
      <form onSubmit={handleSubmit}>
        <div style={{ marginBottom: '20px' }}>
          <label htmlFor="token" style={{ 
            display: 'block', 
            marginBottom: '8px',
            fontSize: '14px',
            fontWeight: 500,
            color: 'var(--color-text-primary)'
          }}>
            Bearer Token
          </label>
          <input
            type="text"
            id="token"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="Enter your OAuth 2.0 bearer token"
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
        <button
          type="submit"
          style={{
            width: '100%',
            padding: '12px',
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
          Login
        </button>
      </form>
      <div style={{ 
        marginTop: '24px', 
        padding: '16px', 
        backgroundColor: 'var(--color-bg-secondary)', 
        borderRadius: 'var(--radius-md)',
        border: '1px solid var(--color-border-light)'
      }}>
        <h3 style={{ 
          marginTop: 0,
          marginBottom: '12px',
          fontSize: '13px',
          fontWeight: 600,
          color: 'var(--color-text-primary)',
          textTransform: 'uppercase',
          letterSpacing: '0.5px'
        }}>
          How to get a token
        </h3>
        <ol style={{ 
          fontSize: '13px', 
          paddingLeft: '20px',
          margin: 0,
          color: 'var(--color-text-secondary)',
          lineHeight: '1.6'
        }}>
          <li>Obtain a bearer token from your OAuth 2.0 provider</li>
          <li>Paste it in the field above</li>
          <li>The token will be stored in your browser's local storage</li>
        </ol>
      </div>
    </div>
  );
};

export default Login;
