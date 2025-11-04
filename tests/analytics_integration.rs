//! Integration tests for analytics functionality
//!
//! These tests verify the analytics system works end-to-end with real GeoIP databases
//! when available. Tests will be skipped if databases cannot be downloaded (e.g., in
//! restricted CI environments).

use lynx::analytics::{AnalyticsAggregator, GeoIpService};
use lynx::storage::{SqliteStorage, Storage};
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::time::{sleep, Duration};

// Global OnceCell to ensure databases are downloaded only once across all tests
static DB_PATHS: OnceCell<Option<(PathBuf, PathBuf)>> = OnceCell::const_new();

/// Helper to download GeoIP database
async fn download_db(url: &str, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading from {} to {}", url, path);

    // Create a client that follows redirects
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;

    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;
    tokio::fs::write(path, bytes).await?;
    Ok(())
}

/// Initialize and download GeoIP databases once
async fn init_dbs() -> Option<(PathBuf, PathBuf)> {
    let temp = std::env::temp_dir();
    let city = temp.join("GeoLite2-City.mmdb");
    let asn = temp.join("GeoLite2-ASN.mmdb");

    // Check if databases already exist and are valid
    if city.exists() && asn.exists() {
        // Quick validation: try to read as MaxMind DB
        if let (Ok(_), Ok(_)) = (
            maxminddb::Reader::open_readfile(&city),
            maxminddb::Reader::open_readfile(&asn),
        ) {
            println!("Using existing GeoIP databases from cache");
            return Some((city, asn));
        } else {
            println!("Cached databases are invalid, re-downloading...");
        }
    }

    // Try to download
    println!("Downloading GeoIP databases...");
    let c_result = download_db(
        "https://s.joefang.org/GeoLite2-City",
        city.to_str().unwrap(),
    )
    .await;
    let a_result = download_db("https://s.joefang.org/GeoLite2-ASN", asn.to_str().unwrap()).await;

    if let Err(e) = &c_result {
        println!("City download error: {}", e);
    }
    if let Err(e) = &a_result {
        println!("ASN download error: {}", e);
    }

    if c_result.is_ok() && a_result.is_ok() {
        Some((city, asn))
    } else {
        println!("Could not download databases - tests will be skipped");
        None
    }
}

/// Get GeoIP database paths, downloading if necessary (thread-safe)
async fn get_dbs() -> Option<(PathBuf, PathBuf)> {
    DB_PATHS
        .get_or_init(|| async { init_dbs().await })
        .await
        .clone()
}

#[tokio::test]
async fn test_geoip_lookups() {
    let Some((city, asn)) = get_dbs().await else {
        println!("SKIPPED: GeoIP databases not available");
        return;
    };

    let geoip = GeoIpService::new(Some(city.to_str().unwrap()), Some(asn.to_str().unwrap()))
        .expect("Failed to create GeoIP service");

    // Test Google DNS IPv4
    let ip: IpAddr = "8.8.8.8".parse().unwrap();
    let geo = geoip.lookup(ip);
    println!("8.8.8.8: {:?}", geo);

    assert_eq!(geo.ip_version, 4);
    assert_eq!(geo.country_code, Some("US".to_string()));
    assert!(geo.asn.is_some(), "Should have ASN for 8.8.8.8");
    assert!(geo.asn_org.is_some(), "Should have ASN org");

    // Test Google DNS IPv6
    let ip6: IpAddr = "2001:4860:4860::8888".parse().unwrap();
    let geo6 = geoip.lookup(ip6);
    println!("2001:4860:4860::8888: {:?}", geo6);

    assert_eq!(geo6.ip_version, 6);
    assert_eq!(geo6.country_code, Some("US".to_string()));
    assert!(geo6.asn.is_some(), "Should have ASN for IPv6");
}

#[tokio::test]
async fn test_storage_integration() {
    let Some((city, asn)) = get_dbs().await else {
        println!("SKIPPED: GeoIP databases not available");
        return;
    };

    // Setup storage
    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    let storage: Arc<dyn Storage> = Arc::new(storage);

    // Setup GeoIP
    let geoip = Arc::new(
        GeoIpService::new(Some(city.to_str().unwrap()), Some(asn.to_str().unwrap())).unwrap(),
    );

    // Create aggregator
    let agg = Arc::new(AnalyticsAggregator::new());

    // Record analytics for multiple IPs - all with valid country codes to ensure proper aggregation
    // Using distinct IPs from different ASNs and locations to avoid over-aggregation
    let ips = vec![
        "8.8.8.8",              // Google US, AS15169
        "9.9.9.9",              // Quad9 US, AS19281
        "208.67.222.222",       // OpenDNS US, AS36692
        "2001:4860:4860::8888"  // Google IPv6 US, AS15169
    ];
    for ip_str in ips {
        let ip: IpAddr = ip_str.parse().unwrap();
        let geo = geoip.lookup(ip);
        let rec = lynx::analytics::AnalyticsRecord {
            short_code: "test123".to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            geo_location: geo,
            client_ip: Some(ip),
        };
        agg.record(rec);
    }

    println!("Aggregator has {} entries", agg.len());
    assert!(!agg.is_empty());

    // Drain and save to storage
    let entries = agg.drain();
    let records: Vec<_> = entries
        .into_iter()
        .map(|(k, v)| {
            (
                k.short_code,
                k.time_bucket,
                k.country_code,
                k.region,
                k.city,
                k.asn.map(|a| a as i64),
                k.ip_version as i32,
                v.count as i64,
            )
        })
        .collect();

    storage.upsert_analytics_batch(records).await.unwrap();

    // Query back
    let analytics = storage
        .get_analytics("test123", None, None, 100)
        .await
        .unwrap();
    println!("Retrieved {} analytics entries", analytics.len());
    assert!(!analytics.is_empty());

    // Verify dimensions
    let has_ipv4 = analytics.iter().any(|a| a.ip_version == 4);
    let has_ipv6 = analytics.iter().any(|a| a.ip_version == 6);
    assert!(has_ipv4);
    assert!(has_ipv6);

    let has_us = analytics
        .iter()
        .any(|a| a.country_code == Some("US".to_string()));
    assert!(has_us);

    // Test aggregation by country
    let agg_country = storage
        .get_analytics_aggregate("test123", None, None, "country", 10)
        .await
        .unwrap();
    println!("Country aggregates: {:?}", agg_country);
    assert!(!agg_country.is_empty());

    // Verify totals match
    let total_agg: i64 = agg_country.iter().map(|a| a.visit_count).sum();
    let total_detail: i64 = analytics.iter().map(|a| a.visit_count).sum();
    assert_eq!(total_agg, total_detail);
}

#[tokio::test]
async fn test_auto_flush() {
    let Some((city, asn)) = get_dbs().await else {
        println!("SKIPPED: GeoIP databases not available");
        return;
    };

    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    let storage: Arc<dyn Storage> = Arc::new(storage);

    let geoip = Arc::new(
        GeoIpService::new(Some(city.to_str().unwrap()), Some(asn.to_str().unwrap())).unwrap(),
    );

    let agg = Arc::new(AnalyticsAggregator::new());

    // Start auto-flush with 2 second interval
    let storage_clone = Arc::clone(&storage);
    let _handle = agg.start_flush_task_with_storage(2, move |entries| {
        let s = Arc::clone(&storage_clone);
        Box::pin(async move {
            if entries.is_empty() {
                return;
            }
            let recs: Vec<_> = entries
                .into_iter()
                .map(|(k, v)| {
                    (
                        k.short_code,
                        k.time_bucket,
                        k.country_code,
                        k.region,
                        k.city,
                        k.asn.map(|a| a as i64),
                        k.ip_version as i32,
                        v.count as i64,
                    )
                })
                .collect();
            let _ = s.upsert_analytics_batch(recs).await;
        })
    });

    // Record analytics
    for i in 0..10 {
        let ip: IpAddr = "8.8.8.8".parse().unwrap();
        let geo = geoip.lookup(ip);
        let rec = lynx::analytics::AnalyticsRecord {
            short_code: format!("auto{}", i % 3),
            timestamp: chrono::Utc::now().timestamp(),
            geo_location: geo,
            client_ip: Some(ip),
        };
        agg.record(rec);
    }

    println!("Recorded 10 entries, waiting for auto-flush...");
    sleep(Duration::from_secs(3)).await;

    // Verify data was flushed
    let mut total = 0;
    for i in 0..3 {
        let code = format!("auto{}", i);
        let analytics = storage.get_analytics(&code, None, None, 100).await.unwrap();
        if !analytics.is_empty() {
            let count: i64 = analytics.iter().map(|a| a.visit_count).sum();
            println!("{}: {} visits", code, count);
            total += count;
        }
    }
    assert!(total > 0, "Should have flushed data");
}

#[tokio::test]
async fn test_aggregation_dimensions() {
    let Some((city, asn)) = get_dbs().await else {
        println!("SKIPPED: GeoIP databases not available");
        return;
    };

    let storage = SqliteStorage::new("sqlite::memory:", 5).await.unwrap();
    storage.init().await.unwrap();
    let storage: Arc<dyn Storage> = Arc::new(storage);

    let geoip = Arc::new(
        GeoIpService::new(Some(city.to_str().unwrap()), Some(asn.to_str().unwrap())).unwrap(),
    );

    let agg = Arc::new(AnalyticsAggregator::new());

    // Record from different IPs with known counts
    // Use IPs with different ASNs and/or locations to avoid aggregation into same key
    let tests = vec![
        ("8.8.8.8", 5),         // Google US, AS15169
        ("9.9.9.9", 3),         // Quad9 US, AS19281 (different ASN)
        ("208.67.222.222", 2)   // OpenDNS US, AS36692 (different ASN)
    ];
    for (ip_str, count) in tests {
        for _ in 0..count {
            let ip: IpAddr = ip_str.parse().unwrap();
            let geo = geoip.lookup(ip);
            let rec = lynx::analytics::AnalyticsRecord {
                short_code: "multi".to_string(),
                timestamp: chrono::Utc::now().timestamp(),
                geo_location: geo,
                client_ip: Some(ip),
            };
            agg.record(rec);
        }
    }

    // Flush to storage
    let entries = agg.drain();
    let records: Vec<_> = entries
        .into_iter()
        .map(|(k, v)| {
            (
                k.short_code,
                k.time_bucket,
                k.country_code,
                k.region,
                k.city,
                k.asn.map(|a| a as i64),
                k.ip_version as i32,
                v.count as i64,
            )
        })
        .collect();
    storage.upsert_analytics_batch(records).await.unwrap();

    // Test different dimensions
    for dim in &["country", "asn", "hour"] {
        let agg = storage
            .get_analytics_aggregate("multi", None, None, dim, 10)
            .await
            .unwrap();
        println!("Aggregated by {}: {:?}", dim, agg);
        assert!(!agg.is_empty());

        let total: i64 = agg.iter().map(|a| a.visit_count).sum();
        assert_eq!(total, 10, "Total should be 10 for {}", dim);
    }
}

#[test]
fn test_ip_anonymization() {
    use lynx::analytics::ip_extractor::anonymize_ip;

    let ip4: IpAddr = "192.168.1.100".parse().unwrap();
    let anon4 = anonymize_ip(ip4);
    assert_eq!(anon4.to_string(), "192.168.1.0");

    let ip6: IpAddr = "2001:db8:85a3::8a2e:370:7334".parse().unwrap();
    let anon6 = anonymize_ip(ip6);
    assert!(anon6.to_string().starts_with("2001:db8:85a3::"));
}
