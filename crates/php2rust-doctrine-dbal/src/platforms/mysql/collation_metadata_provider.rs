//! Port of Doctrine\DBAL\Platforms\MySQL\CollationMetadataProvider.php.
//!
//! `@internal`. A `Result`: the connection-backed provider queries the
//! database.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CollationMetadataProvider.php

use crate::exception::Exception;

/// `@internal`
pub trait CollationMetadataProvider {
    /// `getCollationCharset($collation)`: `?non-empty-string`.
    fn get_collation_charset(&self, collation: &str) -> Result<Option<String>, Exception>;
}
