//! Port of `Doctrine\DBAL\Tools\Console\ConnectionProvider\SingleConnectionProvider`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Tools/Console/ConnectionProvider/SingleConnectionProvider.php

use std::sync::Arc;

use super::ConnectionProvider;
use crate::Connection;
use crate::tools::console::ConnectionNotFound;

/// `SingleConnectionProvider`: one connection, under one name.
pub struct SingleConnectionProvider {
    connection: Arc<Connection>,
    default_connection_name: String,
}

impl SingleConnectionProvider {
    /// `new SingleConnectionProvider($connection)`: the name is `default`.
    pub fn new(connection: Arc<Connection>) -> Self {
        Self::with_name(connection, "default")
    }

    /// `new SingleConnectionProvider($connection, $defaultConnectionName)`.
    pub fn with_name(
        connection: Arc<Connection>,
        default_connection_name: impl Into<String>,
    ) -> Self {
        Self {
            connection,
            default_connection_name: default_connection_name.into(),
        }
    }
}

impl ConnectionProvider for SingleConnectionProvider {
    fn get_default_connection(&self) -> Arc<Connection> {
        Arc::clone(&self.connection)
    }

    fn get_connection(&self, name: &str) -> Result<Arc<Connection>, ConnectionNotFound> {
        if name != self.default_connection_name {
            return Err(ConnectionNotFound::new(format!(
                "Connection with name \"{name}\" does not exist."
            )));
        }

        Ok(Arc::clone(&self.connection))
    }
}
