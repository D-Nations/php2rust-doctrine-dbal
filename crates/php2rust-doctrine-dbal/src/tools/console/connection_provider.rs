//! Port of `Doctrine\DBAL\Tools\Console\ConnectionProvider`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Tools/Console/ConnectionProvider.php

pub mod single_connection_provider;

use std::sync::Arc;

use super::ConnectionNotFound;
use crate::Connection;

/// `ConnectionProvider`: where a command gets its connection.
pub trait ConnectionProvider: Send + Sync {
    /// `getDefaultConnection()`.
    fn get_default_connection(&self) -> Arc<Connection>;

    /// `getConnection($name)`; a `ConnectionNotFound` when no connection
    /// has that name.
    fn get_connection(&self, name: &str) -> Result<Arc<Connection>, ConnectionNotFound>;
}
