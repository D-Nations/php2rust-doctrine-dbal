//! Port of Doctrine\DBAL\Platforms\MySQL\DefaultTableOptions.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/DefaultTableOptions.php

/// `@internal`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultTableOptions {
    charset: String,
    collation: String,
}

impl DefaultTableOptions {
    pub fn new(charset: impl Into<String>, collation: impl Into<String>) -> Self {
        Self {
            charset: charset.into(),
            collation: collation.into(),
        }
    }

    pub fn get_charset(&self) -> &str {
        &self.charset
    }

    pub fn get_collation(&self) -> &str {
        &self.collation
    }
}
