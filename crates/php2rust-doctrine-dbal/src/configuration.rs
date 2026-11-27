//! Port of Doctrine\DBAL\Configuration.php: what the connection reads of
//! it, and the result cache (a PSR-6 pool, compared by identity). The
//! schema manager factory and the driver middlewares are not ported yet
//! (their components are not). The schema
//! assets filter is a `Fn(&str) -> bool` (PHP's callable takes the asset
//! name); configurations compare it by identity.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Configuration.php

use std::fmt;
use std::sync::Arc;

use crate::Exception;
use crate::cache::CacheItemPool;

/// The callable that filters schema assets by name.
pub type SchemaAssetsFilter = Arc<dyn Fn(&str) -> bool + Send + Sync>;

/// Configuration container for the Doctrine DBAL.
#[derive(Clone)]
pub struct Configuration {
    /// The default auto-commit mode for connections.
    auto_commit: bool,
    // Slice SchemaManager: the callable to use to filter schema assets.
    schema_assets_filter: SchemaAssetsFilter,
    /// The cache driver implementation that is used for query result
    /// caching.
    result_cache: Option<Arc<dyn CacheItemPool>>,
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Configuration")
            .field("auto_commit", &self.auto_commit)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Configuration {
    fn eq(&self, other: &Self) -> bool {
        self.auto_commit == other.auto_commit
            && Arc::ptr_eq(&self.schema_assets_filter, &other.schema_assets_filter)
            && match (&self.result_cache, &other.result_cache) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

impl Eq for Configuration {}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

impl Configuration {
    pub fn new() -> Self {
        Self {
            auto_commit: true,
            schema_assets_filter: Arc::new(|_| true),
            result_cache: None,
        }
    }

    /// Gets the cache driver implementation that is used for query result
    /// caching.
    pub fn get_result_cache(&self) -> Option<&Arc<dyn CacheItemPool>> {
        self.result_cache.as_ref()
    }

    /// Sets the cache driver implementation that is used for query result
    /// caching.
    pub fn set_result_cache(&mut self, cache: Arc<dyn CacheItemPool>) {
        self.result_cache = Some(cache);
    }

    // Slice SchemaManager.

    /// Sets the callable to use to filter schema assets.
    pub fn set_schema_assets_filter(&mut self, schema_assets_filter: SchemaAssetsFilter) {
        self.schema_assets_filter = schema_assets_filter;
    }

    /// Returns the callable to use to filter schema assets.
    pub fn get_schema_assets_filter(&self) -> &SchemaAssetsFilter {
        &self.schema_assets_filter
    }

    /// Sets the default auto-commit mode for connections.
    ///
    /// If a connection is in auto-commit mode, then all its SQL statements
    /// will be executed and committed as individual transactions.
    /// Otherwise, its SQL statements are grouped into transactions that are
    /// terminated by a call to either the method commit or the method
    /// rollback. By default, new connections are in auto-commit mode.
    pub fn set_auto_commit(&mut self, auto_commit: bool) {
        self.auto_commit = auto_commit;
    }

    /// Returns the default auto-commit mode for connections.
    pub fn get_auto_commit(&self) -> bool {
        self.auto_commit
    }

    pub fn get_disable_type_comments(&self) -> bool {
        true
    }

    pub fn set_disable_type_comments(
        &mut self,
        disable_type_comments: bool,
    ) -> Result<&mut Self, Exception> {
        if !disable_type_comments {
            return Err(Exception::invalid_argument(
                "Column comments cannot be enabled anymore.",
            ));
        }
        Ok(self)
    }
}
