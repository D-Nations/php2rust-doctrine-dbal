//! Port of Doctrine\DBAL\Driver\Middleware\AbstractConnectionMiddleware.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Middleware/AbstractConnectionMiddleware.php

use std::sync::Arc;

use php_value::{PhpString, Value};

use crate::Exception;
use crate::driver::{BoxFuture, Connection, NativeConnection, Result, Statement};

/// `AbstractConnectionMiddleware`: a driver connection that delegates to
/// the connection it wraps.
#[derive(Clone)]
pub struct AbstractConnectionMiddleware {
    wrapped_connection: Arc<dyn Connection>,
}

impl AbstractConnectionMiddleware {
    pub const CLASS: &'static str =
        "Doctrine\\DBAL\\Driver\\Middleware\\AbstractConnectionMiddleware";

    pub fn new(wrapped_connection: Arc<dyn Connection>) -> Self {
        Self { wrapped_connection }
    }
}

impl Connection for AbstractConnectionMiddleware {
    fn prepare<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, std::result::Result<Box<dyn Statement>, Exception>> {
        self.wrapped_connection.prepare(sql)
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, std::result::Result<Box<dyn Result>, Exception>> {
        self.wrapped_connection.query(sql)
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        self.wrapped_connection.quote(value)
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, std::result::Result<i64, Exception>> {
        self.wrapped_connection.exec(sql)
    }

    fn last_insert_id(&self) -> BoxFuture<'_, std::result::Result<Value, Exception>> {
        self.wrapped_connection.last_insert_id()
    }

    fn begin_transaction(&self) -> BoxFuture<'_, std::result::Result<(), Exception>> {
        self.wrapped_connection.begin_transaction()
    }

    fn commit(&self) -> BoxFuture<'_, std::result::Result<(), Exception>> {
        self.wrapped_connection.commit()
    }

    fn roll_back(&self) -> BoxFuture<'_, std::result::Result<(), Exception>> {
        self.wrapped_connection.roll_back()
    }

    fn get_server_version(&self) -> BoxFuture<'_, std::result::Result<String, Exception>> {
        self.wrapped_connection.get_server_version()
    }

    fn get_native_connection(&self) -> NativeConnection {
        self.wrapped_connection.get_native_connection()
    }
}
