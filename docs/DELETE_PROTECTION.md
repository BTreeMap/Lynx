# URL Table Delete Protection

## Overview

As a critical security measure, the `urls` table in Lynx is protected from accidental or malicious deletion operations. URLs can only be **deactivated** or **reactivated**, never deleted. This ensures data integrity and prevents permanent data loss.

## Implementation

### SQLite

For SQLite databases, deletion protection is implemented using a `BEFORE DELETE` trigger:

```sql
CREATE TRIGGER IF NOT EXISTS prevent_urls_delete
BEFORE DELETE ON urls
FOR EACH ROW
BEGIN
    SELECT RAISE(ABORT, 'DELETE operations are not allowed on the urls table. Use deactivation instead.');
END
```

**How it works:**
- Any attempt to execute `DELETE FROM urls` will be immediately aborted
- The trigger raises an error with a clear message explaining the restriction
- This is enforced at the database level, independent of application code

### PostgreSQL

For PostgreSQL databases, deletion protection uses a multi-layered approach:

#### 1. DELETE Trigger

```sql
CREATE OR REPLACE FUNCTION prevent_urls_delete()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'DELETE operations are not allowed on the urls table. Use deactivation instead.';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER prevent_urls_delete_trigger
BEFORE DELETE ON urls
FOR EACH ROW
EXECUTE FUNCTION prevent_urls_delete();
```

#### 2. TRUNCATE Trigger

```sql
CREATE OR REPLACE FUNCTION prevent_urls_truncate()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'TRUNCATE operations are not allowed on the urls table. Use deactivation instead.';
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER prevent_urls_truncate_trigger
BEFORE TRUNCATE ON urls
FOR EACH STATEMENT
EXECUTE FUNCTION prevent_urls_truncate();
```

#### 3. REVOKE DELETE Permission

As an additional layer of defense, the initialization code attempts to revoke DELETE permissions:

```sql
REVOKE DELETE ON urls FROM PUBLIC;
REVOKE DELETE ON urls FROM CURRENT_USER;
```

**Note:** These REVOKE statements may fail if the database user lacks sufficient privileges, but the triggers provide the primary protection mechanism.

## Usage

### Deactivating URLs

To disable a URL without deleting it:

```rust
// Using the Storage trait
storage.deactivate("short_code").await?;
```

This sets the `is_active` field to `false`, preventing the URL from being used while preserving all historical data.

### Reactivating URLs

To re-enable a previously deactivated URL:

```rust
// Using the Storage trait
storage.reactivate("short_code").await?;
```

This sets the `is_active` field back to `true`.

### Admin Operations

Administrators can:
- ✅ View all URLs
- ✅ Deactivate any URL
- ✅ Reactivate any URL
- ❌ Delete any URL (blocked by database triggers)

## Error Messages

When a DELETE or TRUNCATE operation is attempted, you will receive an error:

**SQLite:**
```
Error: DELETE operations are not allowed on the urls table. Use deactivation instead.
```

**PostgreSQL DELETE:**
```
ERROR: DELETE operations are not allowed on the urls table. Use deactivation instead.
CONTEXT: PL/pgSQL function prevent_urls_delete() line 3 at RAISE
```

**PostgreSQL TRUNCATE:**
```
ERROR: TRUNCATE operations are not allowed on the urls table. Use deactivation instead.
CONTEXT: PL/pgSQL function prevent_urls_truncate() line 3 at RAISE
```

## Testing

The delete protection is thoroughly tested in the integration test suite:

- `test_sqlite_delete_protection` - Verifies SQLite DELETE blocking
- `test_postgres_delete_protection` - Verifies PostgreSQL DELETE blocking
- `test_postgres_truncate_protection` - Verifies PostgreSQL TRUNCATE blocking

To run these tests:

```bash
# SQLite tests (no setup required)
cargo test --test storage_integration_test test_sqlite_delete_protection

# PostgreSQL tests (requires DATABASE_URL)
DATABASE_URL="postgres://user:pass@localhost/db" cargo test --test storage_integration_test test_postgres
```

## Migration

The delete protection triggers are created automatically during the `init()` phase of storage initialization. When upgrading an existing database:

1. The triggers are created idempotently (won't fail if they already exist)
2. No data migration is required
3. Existing URLs remain unchanged
4. Deactivation continues to work as before

## Security Benefits

1. **Prevents Accidental Data Loss** - No risk of accidentally running `DELETE FROM urls`
2. **Protects Against SQL Injection** - Even if an SQL injection vulnerability exists, attackers cannot delete URLs
3. **Audit Trail Preservation** - All URL history is retained for compliance and analysis
4. **Defense in Depth** - Multiple layers (triggers + permissions) provide redundancy
5. **Clear Error Messages** - Failed deletion attempts provide actionable guidance

## Architecture Decision

**Why deactivation instead of deletion?**

- **Data Integrity**: Maintains referential integrity with analytics and click data
- **Audit Trail**: Preserves complete history for compliance and debugging
- **Recovery**: Allows URLs to be reactivated if disabled by mistake
- **Analytics**: Historical data remains available for long-term analysis
- **Simplicity**: Simpler than implementing soft deletes with cascading rules

## See Also

- [Storage Architecture](IMPLEMENTATION_SUMMARY.md)
- [Analytics](ANALYTICS.md)
- [Performance Optimizations](PERFORMANCE_OPTIMIZATIONS.md)
