//! Port of `Doctrine\DBAL\Tools\Console\ConnectionNotFound`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Tools/Console/ConnectionNotFound.php

use std::fmt;

/// `ConnectionNotFound` (extends `OutOfBoundsException`): no connection
/// has the given name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionNotFound {
    message: String,
}

impl ConnectionNotFound {
    /// `ConnectionNotFound::class`.
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Tools\\Console\\ConnectionNotFound";

    /// `new ConnectionNotFound($message)`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// `getMessage()`.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ConnectionNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConnectionNotFound {}
