//! Port of
//! Doctrine\DBAL\Platforms\MySQL\CharsetMetadataProvider\ConnectionCharsetMetadataProvider.php.
//!
//! `@internal`. The provider interface is synchronous (the comparator
//! calls it while it compares), so the query's future is polled once: a
//! driver whose query has to wait on the server refuses with an
//! "unsupported" exception instead of blocking.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CharsetMetadataProvider/ConnectionCharsetMetadataProvider.php

use php_value::{PhpArray, Value};

use super::CharsetMetadataProvider;
use crate::exception::Exception;
use crate::{Connection, ParameterTypes};

/// `@internal`
#[derive(Debug, Clone, Copy)]
pub struct ConnectionCharsetMetadataProvider<'c> {
    connection: &'c Connection,
}

impl<'c> ConnectionCharsetMetadataProvider<'c> {
    pub fn new(connection: &'c Connection) -> Self {
        Self { connection }
    }
}

impl CharsetMetadataProvider for ConnectionCharsetMetadataProvider<'_> {
    fn get_default_charset_collation(&self, charset: &str) -> Result<Option<String>, Exception> {
        fetch_one(
            self.connection,
            "SELECT DEFAULT_COLLATE_NAME
FROM information_schema.CHARACTER_SETS
WHERE CHARACTER_SET_NAME = ?;",
            charset,
        )
    }
}

/// `$connection->fetchOne($sql, [$parameter])` as a `?string`: `false`
/// (no row) is `null`.
pub(crate) fn fetch_one(
    connection: &Connection,
    sql: &str,
    parameter: &str,
) -> Result<Option<String>, Exception> {
    let params = PhpArray::list([Value::string(parameter)]);
    let types = ParameterTypes::new();
    let Some(result) = crate::connection::poll_once(connection.fetch_one(sql, &params, &types))
    else {
        return Err(Exception::new(
            Exception::UNSUPPORTED,
            "the MySQL metadata providers query the database synchronously, and the driver's \
             query has to wait",
        ));
    };

    crate::platforms::sqlite::nullable_text(result?.as_ref())
}
