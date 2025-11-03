# Cloudflare Zero Trust Authentication Setup

This guide explains how to configure Lynx to use Cloudflare Zero Trust (formerly Cloudflare Access) for authentication.

## Overview

Cloudflare Zero Trust provides enterprise-grade authentication with JWT-based tokens. Lynx validates these tokens to authenticate users and can optionally promote specific users to admin roles.

## Prerequisites

1. A Cloudflare account with Zero Trust enabled
2. A domain configured with Cloudflare
3. Lynx deployed behind Cloudflare Zero Trust

## Configuration Steps

### 1. Create a Cloudflare Access Application

1. Log in to your Cloudflare dashboard
2. Navigate to **Zero Trust** → **Access** → **Applications**
3. Click **Add an application**
4. Select **Self-hosted**
5. Configure your application:
   - **Application name**: Lynx URL Shortener
   - **Session duration**: Choose based on your security requirements
   - **Application domain**: Your Lynx instance URL (e.g., `lynx.example.com`)

6. Under **Policies**, create an access policy:
   - **Policy name**: Allow authenticated users
   - **Action**: Allow
   - **Configure rules**: Select your identity providers (e.g., Google, GitHub, Email OTP)

7. Save the application

### 2. Get Your Application Credentials

1. In the application list, click **Configure** on your Lynx application
2. Go to the **Basic information** tab
3. Copy two values:
   - **Application Audience (AUD) Tag**: A long alphanumeric string
   - **Team domain**: Found in Zero Trust settings, looks like `https://your-team-name.cloudflareaccess.com`

### 3. Configure Lynx Environment Variables

Set the following environment variables:

```bash
AUTH_MODE=cloudflare
CLOUDFLARE_TEAM_DOMAIN=https://your-team-name.cloudflareaccess.com
CLOUDFLARE_AUDIENCE=your-application-aud-tag
```

Optional:
```bash
CLOUDFLARE_CERTS_CACHE_SECS=86400  # Default: 24 hours
```

### 4. Restart Lynx

You should see: `Cloudflare Zero Trust authentication enabled`

## Admin Management

Promote users to admin using the CLI:

```bash
# Promote a user
./lynx admin promote <user-sub> cloudflare

# List admins
./lynx admin list

# Demote a user
./lynx admin demote <user-sub> cloudflare
```

**Note:** Admin status from Cloudflare JWT claims takes precedence. Manual promotion only applies when the JWT doesn't grant admin status.

## Migration from auth=none

Legacy URLs will be attributed to:
- User ID: `00000000-0000-0000-0000-000000000000`
- Email: `legacy@nonexistent.joefang.org`

## Additional Resources

- [Cloudflare Zero Trust Documentation](https://developers.cloudflare.com/cloudflare-one/)
- [JWT Validation Guide](https://developers.cloudflare.com/cloudflare-one/identity/authorization-cookie/validating-json/)
