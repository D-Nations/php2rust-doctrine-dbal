//! Port of Doctrine\DBAL\Platforms\MySQL\CharsetMetadataProvider.php.
//!
//! `@internal`. A `Result`: the connection-backed provider queries the
//! database.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CharsetMetadataProvider.php

use crate::exception::Exception;

/// `@internal`
pub trait CharsetMetadataProvider {
    /// `getDefaultCharsetCollation($charset)`: `?non-empty-string`.
    fn get_default_charset_collation(&self, charset: &str) -> Result<Option<String>, Exception>;
}
