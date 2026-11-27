//! The result cache: `Cache\ArrayResult`, `Cache\QueryCacheProfile`, the
//! cache exceptions, and the PSR-6 pool they use ([`pool`]).
//!
//! Upstream: none

mod array_result;
pub(crate) mod cache_exception;
pub mod pool;
mod query_cache_profile;

pub use array_result::ArrayResult;
pub use pool::{
    ArrayAdapter, CacheItem, CacheItemPool, CachedResult, PoolObject, PoolValue, validate_key,
};
pub use query_cache_profile::QueryCacheProfile;
