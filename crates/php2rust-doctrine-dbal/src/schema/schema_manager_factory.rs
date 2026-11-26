//! Port of Doctrine\DBAL\Schema\SchemaManagerFactory.php and
//! DefaultSchemaManagerFactory.php.
//!
//! The factory hands out the concrete [`SQLiteSchemaManager`] (the
//! `AbstractSchemaManager` trait is not object-safe). A connection always
//! uses the default factory: `Configuration::setSchemaManagerFactory()`
//! is not ported.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SchemaManagerFactory.php
//! Upstream: doctrine/dbal src/Schema/DefaultSchemaManagerFactory.php

use crate::Connection;
use crate::exception::Exception;

use super::sqlite_schema_manager::SQLiteSchemaManager;

/// Creates a schema manager for the given connection.
///
/// This interface is an extension point for applications that need to
/// override schema managers.
#[allow(
    async_fn_in_trait,
    reason = "the futures are awaited where they are made"
)]
pub trait SchemaManagerFactory {
    async fn create_schema_manager<'c>(
        &self,
        connection: &'c Connection,
    ) -> Result<SQLiteSchemaManager<'c>, Exception>;
}

/// A schema manager factory that returns the default schema manager for
/// the given platform.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DefaultSchemaManagerFactory;

impl SchemaManagerFactory for DefaultSchemaManagerFactory {
    /// Fails if the platform does not support creating schema managers
    /// yet.
    async fn create_schema_manager<'c>(
        &self,
        connection: &'c Connection,
    ) -> Result<SQLiteSchemaManager<'c>, Exception> {
        connection
            .get_database_platform()
            .await?
            .create_schema_manager(connection)
    }
}
