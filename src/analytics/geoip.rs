//! GeoIP lookup service using MaxMind GeoLite2/GeoIP2 MMDB
//!
//! This module provides thread-safe, high-performance IP geolocation
//! using memory-mapped MaxMind databases.

use anyhow::{Context, Result};
use maxminddb::{geoip2, Mmap, Reader};
use std::net::IpAddr;
use std::sync::Arc;

use crate::analytics::models::GeoLocation;

/// GeoIP lookup service
pub struct GeoIpService {
    reader: Arc<Reader<Mmap>>,
}

impl GeoIpService {
    /// Create a new GeoIP service from an MMDB file path
    ///
    /// # Arguments
    /// * `path` - Path to the MaxMind GeoLite2 or GeoIP2 .mmdb file
    ///
    /// # Returns
    /// A new GeoIpService instance with memory-mapped database
    pub fn new(path: &str) -> Result<Self> {
        let reader = Reader::open_mmap(path)
            .with_context(|| format!("Failed to open GeoIP database at {}", path))?;
        
        Ok(Self {
            reader: Arc::new(reader),
        })
    }

    /// Lookup geographic location for an IP address
    ///
    /// # Arguments
    /// * `ip` - IP address to lookup (IPv4 or IPv6)
    ///
    /// # Returns
    /// GeoLocation information if found, or a default/unknown location
    pub fn lookup(&self, ip: IpAddr) -> GeoLocation {
        let ip_version = match ip {
            IpAddr::V4(_) => 4,
            IpAddr::V6(_) => 6,
        };

        // Try to lookup city information (which includes country)
        if let Ok(city_opt) = self.reader.lookup::<geoip2::City>(ip) {
            if let Some(city) = city_opt {
                return self.extract_from_city(city, ip_version);
            }
        }

        // Fallback: try country-only lookup
        if let Ok(country_opt) = self.reader.lookup::<geoip2::Country>(ip) {
            if let Some(country) = country_opt {
                return self.extract_from_country(country, ip_version);
            }
        }

        // If all lookups fail, return default/unknown location
        GeoLocation {
            country_code: None,
            country_name: None,
            region: None,
            city: None,
            asn: None,
            asn_org: None,
            ip_version,
        }
    }

    /// Extract location from City data
    fn extract_from_city(&self, city: geoip2::City, ip_version: u8) -> GeoLocation {
        let country_code = city
            .country
            .as_ref()
            .and_then(|c| c.iso_code)
            .map(|s| s.to_string());

        let country_name = city
            .country
            .as_ref()
            .and_then(|c| c.names.as_ref())
            .and_then(|names| names.get("en"))
            .map(|s| s.to_string());

        let region = city
            .subdivisions
            .as_ref()
            .and_then(|subdivisions| subdivisions.first())
            .and_then(|subdivision| subdivision.names.as_ref())
            .and_then(|names| names.get("en"))
            .map(|s| s.to_string());

        let city_name = city
            .city
            .as_ref()
            .and_then(|c| c.names.as_ref())
            .and_then(|names| names.get("en"))
            .map(|s| s.to_string());

        GeoLocation {
            country_code,
            country_name,
            region,
            city: city_name,
            asn: None, // ASN requires separate database
            asn_org: None,
            ip_version,
        }
    }

    /// Extract location from Country data (when City is not available)
    fn extract_from_country(&self, country: geoip2::Country, ip_version: u8) -> GeoLocation {
        let country_code = country
            .country
            .as_ref()
            .and_then(|c| c.iso_code)
            .map(|s| s.to_string());

        let country_name = country
            .country
            .as_ref()
            .and_then(|c| c.names.as_ref())
            .and_then(|names| names.get("en"))
            .map(|s| s.to_string());

        GeoLocation {
            country_code,
            country_name,
            region: None,
            city: None,
            asn: None,
            asn_org: None,
            ip_version,
        }
    }
}

// Implement Clone by cloning the Arc
impl Clone for GeoIpService {
    fn clone(&self) -> Self {
        Self {
            reader: Arc::clone(&self.reader),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require an actual MMDB file to run
    // They are mainly for documentation and would need a test database
    
    #[test]
    fn test_geoip_service_creation_invalid_path() {
        let result = GeoIpService::new("/nonexistent/path.mmdb");
        assert!(result.is_err());
    }
}
