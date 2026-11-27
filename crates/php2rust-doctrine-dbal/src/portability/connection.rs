//! Port of Doctrine\DBAL\Portability\Connection.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/Connection.php

use std::sync::Arc;

use php_value::{PhpString, Value};

use super::{Converter, Result, Statement};
use crate::Exception;
use crate::driver::middleware::AbstractConnectionMiddleware;
use crate::driver::{
    BoxFuture, Connection as ConnectionInterface, NativeConnection, Result as ResultInterface,
    Statement as DriverStatement,
};

/// `final class Connection extends AbstractConnectionMiddleware`: the
/// portability wrapper for a connection.
pub struct Connection {
    parent: AbstractConnectionMiddleware,
    converter: Arc<Converter>,
}

impl Connection {
    pub const PORTABILITY_ALL: i64 = 255;
    pub const PORTABILITY_NONE: i64 = 0;
    pub const PORTABILITY_RTRIM: i64 = 1;
    pub const PORTABILITY_EMPTY_TO_NULL: i64 = 4;
    pub const PORTABILITY_FIX_CASE: i64 = 8;

    /// `__construct($connection, $converter)`.
    pub fn new(connection: Arc<dyn ConnectionInterface>, converter: Arc<Converter>) -> Self {
        Self {
            parent: AbstractConnectionMiddleware::new(connection),
            converter,
        }
    }
}

impl ConnectionInterface for Connection {
    fn prepare<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, std::result::Result<Box<dyn DriverStatement>, Exception>> {
        Box::pin(async move {
            let statement = self.parent.prepare(sql).await?;
            Ok(
                Box::new(Statement::new(statement, Arc::clone(&self.converter)))
                    as Box<dyn DriverStatement>,
            )
        })
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, std::result::Result<Box<dyn ResultInterface>, Exception>> {
        Box::pin(async move {
            let result = self.parent.query(sql).await?;
            Ok(Box::new(Result::new(result, Arc::clone(&self.converter)))
                as Box<dyn ResultInterface>)
        })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        self.parent.quote(value)
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, std::result::Result<i64, Exception>> {
        self.parent.exec(sql)
    }

    fn last_insert_id(&self) -> BoxFuture<'_, std::result::Result<Value, Exception>> {
        self.parent.last_insert_id()
    }

    fn begin_transaction(&self) -> BoxFuture<'_, std::result::Result<(), Exception>> {
        self.parent.begin_transaction()
    }

    fn commit(&self) -> BoxFuture<'_, std::result::Result<(), Exception>> {
        self.parent.commit()
    }

    fn roll_back(&self) -> BoxFuture<'_, std::result::Result<(), Exception>> {
        self.parent.roll_back()
    }

    fn get_server_version(&self) -> BoxFuture<'_, std::result::Result<String, Exception>> {
        self.parent.get_server_version()
    }

    fn get_native_connection(&self) -> NativeConnection {
        self.parent.get_native_connection()
    }
}
