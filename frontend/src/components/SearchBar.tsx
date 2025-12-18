import React, { useState, useCallback } from 'react';

interface SearchBarProps {
  onSearch: (query: string) => void;
  onClear: () => void;
  isSearching: boolean;
  placeholder?: string;
}

const SearchBar: React.FC<SearchBarProps> = ({
  onSearch,
  onClear,
  isSearching,
  placeholder = 'Search by short code or URL...',
}) => {
  const [query, setQuery] = useState('');

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      const trimmed = query.trim();
      if (trimmed) {
        onSearch(trimmed);
      }
    },
    [query, onSearch]
  );

  const handleClear = useCallback(() => {
    setQuery('');
    onClear();
  }, [onClear]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        handleClear();
      }
    },
    [handleClear]
  );

  return (
    <form onSubmit={handleSubmit} style={{ marginBottom: '24px' }}>
      <div
        style={{
          display: 'flex',
          gap: '12px',
          alignItems: 'center',
        }}
      >
        <div style={{ flex: 1, position: 'relative' }}>
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={placeholder}
            style={{
              width: '100%',
              padding: '12px 16px',
              paddingRight: query ? '40px' : '16px',
              fontSize: '14px',
              border: '1px solid var(--color-border)',
              borderRadius: 'var(--radius-md)',
              backgroundColor: 'var(--color-bg-elevated)',
              color: 'var(--color-text-primary)',
              outline: 'none',
              boxSizing: 'border-box',
            }}
          />
          {query && (
            <button
              type="button"
              onClick={handleClear}
              aria-label="Clear search"
              style={{
                position: 'absolute',
                right: '12px',
                top: '50%',
                transform: 'translateY(-50%)',
                background: 'none',
                border: 'none',
                cursor: 'pointer',
                color: 'var(--color-text-tertiary)',
                fontSize: '18px',
                padding: '4px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
              }}
              title="Clear search"
            >
              ×
            </button>
          )}
        </div>
        <button
          type="submit"
          disabled={isSearching || !query.trim()}
          style={{
            padding: '12px 24px',
            backgroundColor: 'var(--color-text-primary)',
            color: 'var(--color-bg)',
            border: 'none',
            borderRadius: 'var(--radius-md)',
            fontSize: '14px',
            fontWeight: 500,
            cursor: isSearching || !query.trim() ? 'not-allowed' : 'pointer',
            opacity: isSearching || !query.trim() ? 0.6 : 1,
            whiteSpace: 'nowrap',
          }}
        >
          {isSearching ? 'Searching...' : 'Search'}
        </button>
      </div>
    </form>
  );
};

export default SearchBar;
