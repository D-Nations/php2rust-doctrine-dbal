//! Port of tests/Portability/ConnectionTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. The mock is a driver connection that counts its
//! `getServerVersion()` calls and gives the native connection it holds;
//! `assertSame()` of two objects is `Arc::ptr_eq()`.
//!
//! Upstream: doctrine/dbal tests/Portability/ConnectionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use php_value::{PhpString, Value};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::driver::{
    BoxFuture, Connection as ConnectionInterface, NativeConnection, Result as DriverResult,
    Statement,
};
use php2rust_doctrine_dbal::portability::{Connection, Converter};

/// `createMock(ConnectionInterface::class)`: `getServerVersion()` gives
/// `1.2.3` and `getNativeConnection()` the given object; nothing else is
/// expected to be called.
struct DriverConnectionMock {
    server_version_calls: AtomicUsize,
    native_connection: NativeConnection,
}

impl DriverConnectionMock {
    fn new(native_connection: NativeConnection) -> Arc<Self> {
        Arc::new(Self {
            server_version_calls: AtomicUsize::new(0),
            native_connection,
        })
    }
}

fn unexpected<T>(method: &str) -> Result<T, Exception> {
    Err(Exception::new(
        Exception::LOGIC_EXCEPTION,
        format!("unexpected call to {method}()"),
    ))
}

impl ConnectionInterface for DriverConnectionMock {
    fn prepare<'a>(
        &'a self,
        _sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn Statement>, Exception>> {
        Box::pin(async { unexpected("prepare") })
    }

    fn query<'a>(
        &'a self,
        _sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async { unexpected("query") })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        PhpString::from(value)
    }

    fn exec<'a>(&'a self, _sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async { unexpected("exec") })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async { unexpected("lastInsertId") })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async { unexpected("beginTransaction") })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async { unexpected("commit") })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async { unexpected("rollBack") })
    }

    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        self.server_version_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok("1.2.3".to_owned()) })
    }

    fn get_native_connection(&self) -> NativeConnection {
        Arc::clone(&self.native_connection)
    }
}

/// `new class () {}`.
struct NativeConnectionObject;

fn converter() -> Arc<Converter> {
    Arc::new(Converter::new(false, false, Some(Converter::CASE_LOWER)).unwrap())
}

#[tokio::test]
async fn testGetServerVersion() {
    let driver_connection = DriverConnectionMock::new(Arc::new(NativeConnectionObject));

    let connection = Connection::new(driver_connection.clone(), converter());

    assert_eq!("1.2.3", connection.get_server_version().await.unwrap());
    assert_eq!(
        1,
        driver_connection
            .server_version_calls
            .load(Ordering::SeqCst)
    );
}

#[tokio::test]
async fn testGetNativeConnection() {
    let native_connection: NativeConnection = Arc::new(NativeConnectionObject);

    let driver_connection = DriverConnectionMock::new(Arc::clone(&native_connection));

    let connection = Connection::new(driver_connection, converter());

    assert!(Arc::ptr_eq(
        &native_connection,
        &connection.get_native_connection()
    ));
}
