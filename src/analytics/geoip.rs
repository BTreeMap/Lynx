//! GeoIP lookup service using MaxMind GeoLite2/GeoIP2 MMDB
//!
//! This module provides thread-safe, high-performance IP geolocation
//! using memory-mapped MaxMind databases.

use anyhow::{Context, Result};
use maxminddb::{geoip2, Mmap, Reader};
use std::net::IpAddr;
use std::sync::Arc;

use crate::analytics::models::GeoLocation;

/// GeoIP lookup service that supports both City and ASN databases
pub struct GeoIpService {
    city_reader: Option<Arc<Reader<Mmap>>>,
    asn_reader: Option<Arc<Reader<Mmap>>>,
}

impl GeoIpService {
    /// Create a new GeoIP service from MMDB file paths
    ///
    /// # Arguments
    /// * `city_path` - Optional path to the MaxMind GeoLite2-City or GeoIP2-City .mmdb file
    /// * `asn_path` - Optional path to the MaxMind GeoLite2-ASN .mmdb file
    ///
    /// # Returns
    /// A new GeoIpService instance with memory-mapped databases
    pub fn new(city_path: Option<&str>, asn_path: Option<&str>) -> Result<Self> {
        let city_reader = if let Some(path) = city_path {
            let reader = unsafe { Reader::open_mmap(path) }
                .with_context(|| format!("Failed to open GeoIP City database at {}", path))?;
            Some(Arc::new(reader))
        } else {
            None
        };

        let asn_reader = if let Some(path) = asn_path {
            let reader = unsafe { Reader::open_mmap(path) }
                .with_context(|| format!("Failed to open GeoIP ASN database at {}", path))?;
            Some(Arc::new(reader))
        } else {
            None
        };

        Ok(Self {
            city_reader,
            asn_reader,
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

        let mut geo_location = GeoLocation {
            ip_version,
            ..Default::default()
        };

        // Try to lookup city information (which includes country)
        if let Some(ref reader) = self.city_reader {
            let mut extracted = false;

            // First try City lookup (includes country, region, city)
            if let Ok(result) = reader.lookup(ip) {
                if let Ok(Some(city)) = result.decode::<geoip2::City>() {
                    self.extract_from_city(&city, &mut geo_location);
                    extracted = true;
                }

                // Fallback to Country lookup if City data wasn't available.
                // This works because the City database is a superset of Country data,
                // and geoip2::Country extracts just the country fields from any GeoIP2 database.
                if !extracted {
                    if let Ok(Some(country)) = result.decode::<geoip2::Country>() {
                        self.extract_from_country(&country, &mut geo_location);
                    }
                }
            }
        }

        // Lookup ASN information
        if let Some(ref reader) = self.asn_reader {
            if let Ok(result) = reader.lookup(ip) {
                if let Ok(Some(asn)) = result.decode::<geoip2::Asn>() {
                    geo_location.asn = asn.autonomous_system_number;
                    geo_location.asn_org =
                        asn.autonomous_system_organization.map(|s| s.to_string());
                }
            }
        }

        geo_location
    }

    /// Extract location from City data
    fn extract_from_city(&self, city: &geoip2::City, geo_location: &mut GeoLocation) {
        geo_location.country_code = city.country.iso_code.map(|s| s.to_string());
        geo_location.country_name = city.country.names.english.map(|s| s.to_string());

        if let Some(subdivision) = city.subdivisions.first() {
            geo_location.region = subdivision.names.english.map(|s| s.to_string());
        }

        geo_location.city = city.city.names.english.map(|s| s.to_string());
    }

    /// Extract location from Country data (when City is not available)
    fn extract_from_country(&self, country: &geoip2::Country, geo_location: &mut GeoLocation) {
        geo_location.country_code = country.country.iso_code.map(|s| s.to_string());
        geo_location.country_name = country.country.names.english.map(|s| s.to_string());
    }
}

// Implement Clone by cloning the Arcs
impl Clone for GeoIpService {
    fn clone(&self) -> Self {
        Self {
            city_reader: self.city_reader.clone(),
            asn_reader: self.asn_reader.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require actual MMDB files to run
    // They are mainly for documentation and would need a test database

    #[test]
    fn test_geoip_service_creation_invalid_path() {
        let result = GeoIpService::new(Some("/nonexistent/path.mmdb"), None);
        assert!(result.is_err());
    }

    #[test]
    fn test_geoip_service_creation_no_databases() {
        let result = GeoIpService::new(None, None);
        assert!(result.is_ok());
    }
}
