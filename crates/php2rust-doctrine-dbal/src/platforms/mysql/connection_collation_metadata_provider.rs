//! Port of
//! Doctrine\DBAL\Platforms\MySQL\CollationMetadataProvider\ConnectionCollationMetadataProvider.php.
//!
//! `@internal`. As `ConnectionCharsetMetadataProvider`, the query's
//! future is polled once.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CollationMetadataProvider/ConnectionCollationMetadataProvider.php

use super::CollationMetadataProvider;
use super::connection_charset_metadata_provider::fetch_one;
use crate::Connection;
use crate::exception::Exception;

/// `@internal`
#[derive(Debug, Clone, Copy)]
pub struct ConnectionCollationMetadataProvider<'c> {
    connection: &'c Connection,
}

impl<'c> ConnectionCollationMetadataProvider<'c> {
    pub fn new(connection: &'c Connection) -> Self {
        Self { connection }
    }
}

impl CollationMetadataProvider for ConnectionCollationMetadataProvider<'_> {
    fn get_collation_charset(&self, collation: &str) -> Result<Option<String>, Exception> {
        fetch_one(
            self.connection,
            "SELECT CHARACTER_SET_NAME
FROM information_schema.COLLATIONS
WHERE COLLATION_NAME = ?;",
            collation,
        )
    }
}
