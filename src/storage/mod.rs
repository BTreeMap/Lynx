pub mod cached;
pub mod postgres;
pub mod sqlite;
pub mod trait_def;

#[cfg(test)]
mod patch_tests;

pub use cached::CachedStorage;
pub use postgres::PostgresStorage;
pub use sqlite::SqliteStorage;
pub use trait_def::{LookupMetadata, LookupResult, Storage, StorageError, StorageResult};
