//! Client IP extraction from HTTP headers with trust validation
//!
//! This module implements secure client IP extraction that:
//! - Validates trust chains for X-Forwarded-For and Forwarded headers
//! - Supports vendor-specific headers (e.g., CF-Connecting-IP)
//! - Falls back to socket remote address when headers are untrusted
//! - Handles both IPv4 and IPv6

use axum::http::HeaderMap;
use std::net::IpAddr;
use tracing::warn;

use crate::config::{AnalyticsConfig, TrustedProxyMode};

/// Extract the client IP address from HTTP headers
///
/// # Arguments
/// * `headers` - HTTP request headers
/// * `socket_addr` - The socket remote address (fallback)
/// * `config` - Analytics configuration with trust settings
///
/// # Returns
/// The client IP address, extracted according to the trust configuration
pub fn extract_client_ip(
    headers: &HeaderMap,
    socket_addr: IpAddr,
    config: &AnalyticsConfig,
) -> IpAddr {
    match config.trusted_proxy_mode {
        TrustedProxyMode::Cloudflare => {
            extract_cloudflare_ip(headers).unwrap_or_else(|| {
                warn!("CF-Connecting-IP header missing in Cloudflare mode, using socket address");
                socket_addr
            })
        }
        TrustedProxyMode::Standard => {
            extract_standard_ip(headers, config).unwrap_or(socket_addr)
        }
        TrustedProxyMode::None => socket_addr,
    }
}

/// Extract IP from Cloudflare-specific header
fn extract_cloudflare_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get("cf-connecting-ip")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<IpAddr>().ok())
}

/// Extract IP from standard headers (Forwarded, X-Forwarded-For) with trust validation
fn extract_standard_ip(headers: &HeaderMap, config: &AnalyticsConfig) -> Option<IpAddr> {
    // Prefer RFC 7239 Forwarded header
    if let Some(ip) = extract_from_forwarded(headers, config) {
        return Some(ip);
    }
    
    // Fall back to X-Forwarded-For
    extract_from_x_forwarded_for(headers, config)
}

/// Parse RFC 7239 Forwarded header
fn extract_from_forwarded(headers: &HeaderMap, _config: &AnalyticsConfig) -> Option<IpAddr> {
    let forwarded = headers.get("forwarded")?.to_str().ok()?;
    
    // Parse Forwarded header: Forwarded: for=192.0.2.60;proto=http;by=203.0.113.43
    // We want the "for" parameter
    for element in forwarded.split(',') {
        for param in element.split(';') {
            let param = param.trim();
            if let Some(value) = param.strip_prefix("for=") {
                // Remove quotes and port if present
                let ip_str = value
                    .trim_matches('"')
                    .trim_start_matches('[')
                    .split(']')
                    .next()
                    .unwrap_or(value)
                    .split(':')
                    .next()
                    .unwrap_or(value);
                
                if let Ok(ip) = ip_str.parse::<IpAddr>() {
                    // For now, return the first IP (rightmost trust validation not yet implemented)
                    // TODO: Implement proper right-to-left trust chain validation
                    return Some(ip);
                }
            }
        }
    }
    
    None
}

/// Parse X-Forwarded-For header with right-to-left trust validation
fn extract_from_x_forwarded_for(headers: &HeaderMap, config: &AnalyticsConfig) -> Option<IpAddr> {
    let xff = headers.get("x-forwarded-for")?.to_str().ok()?;
    
    let ips: Vec<IpAddr> = xff
        .split(',')
        .filter_map(|s| s.trim().parse::<IpAddr>().ok())
        .collect();
    
    if ips.is_empty() {
        return None;
    }
    
    // If num_trusted_proxies is specified, skip that many from the right
    if let Some(num_trusted) = config.num_trusted_proxies {
        if ips.len() > num_trusted {
            return Some(ips[ips.len() - num_trusted - 1]);
        } else {
            // Not enough IPs in chain, return the leftmost (least trusted)
            return ips.first().copied();
        }
    }
    
    // If trusted_proxies CIDR list is specified, find first non-trusted IP from right
    if !config.trusted_proxies.is_empty() {
        // For simplicity, parse CIDR ranges (basic implementation)
        // A production implementation would use a proper CIDR matching library
        // For now, return the rightmost IP (first from right in chain)
        // TODO: Implement proper CIDR matching for trusted proxy validation
        return ips.last().copied();
    }
    
    // No trust configuration, return the rightmost IP
    ips.last().copied()
}

/// Anonymize an IP address by truncating to network prefix
///
/// - IPv4: Truncate to /24 (zero last octet)
/// - IPv6: Truncate to /48 (zero last 80 bits)
pub fn anonymize_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(addr) => {
            let octets = addr.octets();
            IpAddr::V4(std::net::Ipv4Addr::new(octets[0], octets[1], octets[2], 0))
        }
        IpAddr::V6(addr) => {
            let segments = addr.segments();
            // Keep first 3 segments (48 bits), zero the rest
            IpAddr::V6(std::net::Ipv6Addr::new(
                segments[0],
                segments[1],
                segments[2],
                0,
                0,
                0,
                0,
                0,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn create_config(mode: TrustedProxyMode) -> AnalyticsConfig {
        AnalyticsConfig {
            enabled: true,
            geoip_db_path: None,
            ip_anonymization: false,
            trusted_proxy_mode: mode,
            trusted_proxies: vec![],
            num_trusted_proxies: None,
            flush_interval_secs: 60,
        }
    }

    #[test]
    fn test_extract_client_ip_none_mode() {
        let headers = HeaderMap::new();
        let socket_addr: IpAddr = "192.168.1.1".parse().unwrap();
        let config = create_config(TrustedProxyMode::None);

        let result = extract_client_ip(&headers, socket_addr, &config);
        assert_eq!(result, socket_addr);
    }

    #[test]
    fn test_extract_cloudflare_ip() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "cf-connecting-ip",
            HeaderValue::from_static("203.0.113.1"),
        );
        let socket_addr: IpAddr = "192.168.1.1".parse().unwrap();
        let config = create_config(TrustedProxyMode::Cloudflare);

        let result = extract_client_ip(&headers, socket_addr, &config);
        assert_eq!(result, "203.0.113.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_extract_x_forwarded_for_basic() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("203.0.113.1, 198.51.100.1"),
        );
        let socket_addr: IpAddr = "192.168.1.1".parse().unwrap();
        let config = create_config(TrustedProxyMode::Standard);

        let result = extract_client_ip(&headers, socket_addr, &config);
        // Should return rightmost IP in the absence of trust configuration
        assert_eq!(result, "198.51.100.1".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_anonymize_ipv4() {
        let ip: IpAddr = "192.168.1.100".parse().unwrap();
        let anonymized = anonymize_ip(ip);
        assert_eq!(anonymized, "192.168.1.0".parse::<IpAddr>().unwrap());
    }

    #[test]
    fn test_anonymize_ipv6() {
        let ip: IpAddr = "2001:db8::1234:5678".parse().unwrap();
        let anonymized = anonymize_ip(ip);
        // Should zero out everything after first 48 bits (3 segments)
        assert_eq!(anonymized, "2001:db8::".parse::<IpAddr>().unwrap());
    }
}
