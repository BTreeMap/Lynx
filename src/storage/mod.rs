pub mod cached;
pub mod postgres;
pub mod sqlite;
pub mod trait_def;

pub use cached::{CachedStorage, RedirectLookup, RedirectTarget};
pub use postgres::PostgresStorage;
pub use sqlite::SqliteStorage;
pub use trait_def::{
    ClickIncrement, LookupMetadata, LookupResult, OwnedClickError, SearchParams, SearchResult,
    Storage, StorageError, StorageResult,
};
