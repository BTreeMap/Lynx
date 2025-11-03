# Analytics Feature Guide

## Overview

Lynx supports optional visitor IP analytics with geolocation capabilities. This feature is designed to be high-performance and does not affect the core URL redirection latency when disabled.

## Features

- **GeoIP Lookup**: Resolve visitor IP addresses to geographic locations (country, region, city)
- **Privacy-Focused**: Optional IP anonymization
- **Flexible Trust Models**: Support for direct connections, standard proxies (X-Forwarded-For/RFC 7239), and CDN-specific headers (Cloudflare)
- **High Performance**: Memory-mapped MaxMind database with in-memory aggregation
- **Non-Blocking**: GeoIP lookups don't block redirects
- **IPv4/IPv6 Support**: Unified handling of both IP versions

## Enabling Analytics

### 1. Compile with Analytics Feature

The analytics feature is optional and must be enabled at compile time:

```bash
cargo build --release --features analytics
```

### 2. Download GeoIP Database

Download the MaxMind GeoLite2 or GeoIP2 database (free or paid):

1. Sign up for a free account at https://www.maxmind.com/en/geolite2/signup
2. Download the GeoLite2 City or Country database in MMDB format
3. Place the `.mmdb` file in a location accessible to Lynx

**Recommended**: GeoLite2-City.mmdb for full geographic resolution including cities.  
**Alternative**: GeoLite2-Country.mmdb for smaller footprint with country-level data only.

### 3. Configure Environment Variables

Add the following to your `.env` file or environment:

```bash
# Enable analytics
ANALYTICS_ENABLED=true

# Path to GeoIP database
ANALYTICS_GEOIP_DB_PATH=/path/to/GeoLite2-City.mmdb

# Optional: Enable IP anonymization (truncate to network prefix)
ANALYTICS_IP_ANONYMIZATION=false

# Trusted proxy mode: none, standard, or cloudflare
ANALYTICS_TRUSTED_PROXY_MODE=none

# Optional: Trusted proxy CIDR ranges (for standard mode)
# ANALYTICS_TRUSTED_PROXIES=10.0.0.0/8,172.16.0.0/12,192.168.0.0/16

# Optional: Number of trusted proxies to skip from right in X-Forwarded-For
# ANALYTICS_NUM_TRUSTED_PROXIES=1

# Optional: Analytics flush interval in seconds (default: 60)
ANALYTICS_FLUSH_INTERVAL_SECS=60
```

## Trust Models

### None (Default)

Use the direct socket remote address only. Ignore all proxy headers.

```bash
ANALYTICS_TRUSTED_PROXY_MODE=none
```

**Use case**: Direct connections without reverse proxies.

### Standard

Trust RFC 7239 `Forwarded` and `X-Forwarded-For` headers with validation.

```bash
ANALYTICS_TRUSTED_PROXY_MODE=standard
```

**Configuration options**:

1. **CIDR-based trust** (recommended for security):
   ```bash
   ANALYTICS_TRUSTED_PROXIES=10.0.0.0/8,172.16.0.0/12
   ```

2. **Count-based trust** (simpler but less secure):
   ```bash
   ANALYTICS_NUM_TRUSTED_PROXIES=1
   ```

**Use case**: Behind nginx, HAProxy, or other standard reverse proxies.

### Cloudflare

Trust only the Cloudflare-specific `CF-Connecting-IP` header.

```bash
ANALYTICS_TRUSTED_PROXY_MODE=cloudflare
```

**Use case**: Traffic exclusively routed through Cloudflare.

**Security Note**: Only use this mode if ALL traffic is guaranteed to pass through Cloudflare and direct origin access is blocked.

## IP Anonymization

When enabled, IP addresses are truncated to network prefixes before storage:

- **IPv4**: `/24` network (e.g., `192.168.1.100` → `192.168.1.0`)
- **IPv6**: `/48` network (e.g., `2001:db8::1234` → `2001:db8::`)

Enable anonymization:

```bash
ANALYTICS_IP_ANONYMIZATION=true
```

**Privacy Note**: When anonymization is enabled, the raw IP address is not stored in analytics records.

## Database Setup

The GeoIP database should be updated periodically to maintain accuracy:

1. **Manual updates**: Download new databases from MaxMind quarterly
2. **Automated updates**: Use [geoipupdate](https://github.com/maxmind/geoipupdate) tool

Example cron job for weekly updates:

```bash
0 3 * * 0 /usr/local/bin/geoipupdate
```

## Performance Characteristics

- **Lookup Latency**: ~1-10 microseconds per IP with memory-mapped database
- **Memory Usage**: Database size (~70MB for City, ~5MB for Country) + in-memory aggregates
- **Redirect Impact**: Minimal (non-blocking, asynchronous aggregation)

## Architecture

### Data Flow

1. Client makes request → Redirect handler
2. Extract client IP based on trust configuration
3. Perform GeoIP lookup (non-blocking)
4. Record in memory aggregator
5. Return redirect response immediately
6. Background task flushes aggregates periodically

### Aggregation

Analytics are aggregated in-memory by:
- Short code
- Time bucket (hourly)
- Country
- Region
- City
- ASN
- IP version

This reduces database write load and improves performance.

## Security Considerations

### Header Spoofing

**Risk**: Clients can inject fake `X-Forwarded-For` or `X-Real-IP` headers.

**Mitigation**:
- Use `ANALYTICS_TRUSTED_PROXY_MODE=none` for direct connections
- Configure reverse proxy to overwrite (not append) client headers
- Use CIDR-based trust lists to validate proxy sources
- Consider vendor-specific modes (Cloudflare) when applicable

### IP Privacy

- Enable `ANALYTICS_IP_ANONYMIZATION` for privacy compliance
- Consider legal requirements (GDPR, CCPA) for IP storage
- Document IP handling in your privacy policy

### Database Security

- Restrict file permissions on `.mmdb` files
- Ensure GeoIP data is from trusted sources only
- Verify database integrity after downloads

## Troubleshooting

### Analytics Not Recording

1. **Check feature is enabled**:
   ```bash
   cargo build --release --features analytics
   ```

2. **Verify configuration**:
   ```bash
   ANALYTICS_ENABLED=true
   ```

3. **Check GeoIP database path**:
   ```bash
   ls -lh /path/to/GeoLite2-City.mmdb
   ```

4. **Review logs** for GeoIP initialization messages:
   ```
   Analytics enabled
      - GeoIP database loaded from: /path/to/GeoLite2-City.mmdb
   ```

### Incorrect Geographic Data

- **Symptom**: IPs resolving to wrong locations
- **Cause**: Outdated GeoIP database
- **Solution**: Download latest database from MaxMind

### Missing Client IPs

- **Symptom**: All IPs show as server IP
- **Cause**: Incorrect trust configuration
- **Solution**: 
  - Check `ANALYTICS_TRUSTED_PROXY_MODE`
  - Verify proxy headers are being sent
  - Ensure proxy is in `ANALYTICS_TRUSTED_PROXIES`

## Example Configurations

### Direct Connection (No Proxy)

```bash
ANALYTICS_ENABLED=true
ANALYTICS_GEOIP_DB_PATH=/var/lib/geoip/GeoLite2-City.mmdb
ANALYTICS_TRUSTED_PROXY_MODE=none
ANALYTICS_IP_ANONYMIZATION=false
```

### Behind Nginx

```bash
ANALYTICS_ENABLED=true
ANALYTICS_GEOIP_DB_PATH=/var/lib/geoip/GeoLite2-City.mmdb
ANALYTICS_TRUSTED_PROXY_MODE=standard
ANALYTICS_TRUSTED_PROXIES=10.0.1.0/24
ANALYTICS_IP_ANONYMIZATION=false
```

Nginx configuration:
```nginx
location / {
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_pass http://lynx:3000;
}
```

### Behind Cloudflare

```bash
ANALYTICS_ENABLED=true
ANALYTICS_GEOIP_DB_PATH=/var/lib/geoip/GeoLite2-City.mmdb
ANALYTICS_TRUSTED_PROXY_MODE=cloudflare
ANALYTICS_IP_ANONYMIZATION=false
```

**Important**: Ensure origin is only accessible via Cloudflare IPs.

### Privacy-Focused Setup

```bash
ANALYTICS_ENABLED=true
ANALYTICS_GEOIP_DB_PATH=/var/lib/geoip/GeoLite2-Country.mmdb
ANALYTICS_TRUSTED_PROXY_MODE=standard
ANALYTICS_NUM_TRUSTED_PROXIES=1
ANALYTICS_IP_ANONYMIZATION=true
```

## Future Enhancements

- Database storage for analytics aggregates
- API endpoints for querying analytics data
- Dashboard visualizations
- ASN enrichment
- Time-series analytics

## References

- [MaxMind GeoLite2 Free Geolocation Data](https://dev.maxmind.com/geoip/geolite2-free-geolocation-data)
- [RFC 7239: Forwarded HTTP Extension](https://datatracker.ietf.org/doc/html/rfc7239)
- [Cloudflare HTTP Headers](https://developers.cloudflare.com/fundamentals/reference/http-headers/)
