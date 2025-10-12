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
    <div style={{ maxWidth: '400px', margin: '50px auto', padding: '20px' }}>
      <h1>Lynx URL Shortener</h1>
      <p style={{ marginBottom: '20px' }}>
        Please enter your OAuth 2.0 bearer token to continue.
      </p>
      <form onSubmit={handleSubmit}>
        <div style={{ marginBottom: '15px' }}>
          <label htmlFor="token" style={{ display: 'block', marginBottom: '5px' }}>
            Bearer Token:
          </label>
          <input
            type="text"
            id="token"
            value={token}
            onChange={(e) => setToken(e.target.value)}
            placeholder="Enter your OAuth 2.0 bearer token"
            style={{
              width: '100%',
              padding: '8px',
              fontSize: '14px',
              border: '1px solid #ccc',
              borderRadius: '4px',
            }}
            required
          />
        </div>
        <button
          type="submit"
          style={{
            width: '100%',
            padding: '10px',
            backgroundColor: '#007bff',
            color: 'white',
            border: 'none',
            borderRadius: '4px',
            fontSize: '16px',
            cursor: 'pointer',
          }}
        >
          Login
        </button>
      </form>
      <div style={{ marginTop: '20px', padding: '15px', backgroundColor: '#f8f9fa', borderRadius: '4px' }}>
        <h3 style={{ marginTop: 0, fontSize: '14px' }}>How to get a token:</h3>
        <ol style={{ fontSize: '13px', paddingLeft: '20px' }}>
          <li>Obtain a bearer token from your OAuth 2.0 provider</li>
          <li>Paste it in the field above</li>
          <li>The token will be stored in your browser's local storage</li>
        </ol>
      </div>
    </div>
  );
};

export default Login;
