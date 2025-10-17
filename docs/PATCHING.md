# Database Patching CLI

This document describes the database patching functionality for fixing malformed `created_by` values in the Lynx URL shortener.

## Problem

When users start with `auth=none`, all URLs are created with a `created_by` value of `00000000-0000-0000-0000-000000000000` (all-zero UUID) or `null`. If users later switch to `auth=oauth` or `auth=cloudflare`, they may see these malformed values in their interface, which can be confusing or problematic for attribution and access control.

## Solution

The `lynx patch` CLI command provides two ways to fix malformed `created_by` values:

### 1. Patch a Single URL

Update the `created_by` field for a specific short link:

```bash
lynx patch link <USER_ID> <SHORT_CODE>
```

**Arguments:**
- `USER_ID`: The user identifier to set as the new `created_by` value
- `SHORT_CODE`: The short code of the URL to patch

**Example:**
```bash
lynx patch link user@example.com mylink
```

This command will:
- Verify that the short code exists
- Display the current `created_by` value
- Update the `created_by` field to the new user ID
- Confirm the update

### 2. Fix All Malformed Entries

Automatically fix all malformed `created_by` values in the database:

```bash
lynx patch fix-all <USER_ID>
```

**Arguments:**
- `USER_ID`: The user identifier to set for all malformed entries

**Example:**
```bash
lynx patch fix-all admin@example.com
```

This command will:
- Find all URLs with malformed `created_by` values (NULL, empty string, or all-zero UUID)
- Update them to the specified user ID
- Report the number of entries fixed
- **Preserve** all valid `created_by` values (does not overwrite legitimate user IDs)

## What is Considered "Malformed"?

The `fix-all` command only patches entries where `created_by` is:
1. `NULL` (no value set)
2. Empty string (`""`)
3. All-zero UUID (`00000000-0000-0000-0000-000000000000`)

All other values are considered valid and will **not** be modified, including:
- Valid UUIDs
- Email addresses
- Custom user identifiers
- Any other non-empty string

## Safety

The patching functionality is designed to be safe:

1. **Single URL patching** only affects the specified short code
2. **Fix-all patching** only updates malformed values, never overwrites valid user data
3. Both commands are tested to ensure they don't accidentally overwrite legitimate user identifiers
4. The commands report what they're doing before and after execution
5. Database transactions ensure atomic updates

## Testing

The feature includes comprehensive unit tests covering:
- Patching single URLs
- Patching all malformed entries
- Verifying that valid user IDs are not overwritten
- Handling non-existent URLs
- Edge cases with empty databases

Run tests with:
```bash
cargo test patch_tests
```

## Examples

### Scenario 1: Fix a Single Migrated URL

You notice one specific URL has the wrong owner after migrating from `auth=none` to OAuth:

```bash
$ lynx patch link alice@example.com abc123
Current created_by: Some("00000000-0000-0000-0000-000000000000")
✓ Updated created_by for short code 'abc123' to 'alice@example.com'
```

### Scenario 2: Clean Up After Migration

After switching from `auth=none` to Cloudflare Zero Trust, you want to assign all legacy URLs to an admin account:

```bash
$ lynx patch fix-all admin@company.com
⚠ This will update all malformed created_by values (NULL, empty string, or all-zero UUID)
   to user_id: 'admin@company.com'

Checking for malformed entries...
✓ Successfully patched 42 malformed created_by value(s) to 'admin@company.com'
```

### Scenario 3: Verify Database is Clean

After fixing issues, verify no malformed entries remain:

```bash
$ lynx patch fix-all admin@company.com
⚠ This will update all malformed created_by values (NULL, empty string, or all-zero UUID)
   to user_id: 'admin@company.com'

Checking for malformed entries...
✓ No malformed created_by values found. Database is clean!
```
