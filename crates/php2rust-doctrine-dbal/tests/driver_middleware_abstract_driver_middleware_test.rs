//! Port of tests/Driver/Middleware/AbstractDriverMiddlewareTest.php.
//! PHPUnit's mock driver is a double that records the parameters it is
//! connected with and returns the stub connection; `assertSame()` on an
//! object compares the addresses.
//!
//! Upstream: doctrine/dbal tests/Driver/Middleware/AbstractDriverMiddlewareTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, PhpString, Value, php_array};
use php2rust_doctrine_dbal::driver::api::ExceptionConverter;
use php2rust_doctrine_dbal::driver::api::sqlite::ExceptionConverter as SQLiteExceptionConverter;
use php2rust_doctrine_dbal::driver::middleware::AbstractDriverMiddleware;
use php2rust_doctrine_dbal::driver::{
    BoxFuture, Connection, Driver, NativeConnection, PlatformBox, Result as DriverResult, Statement,
};
use php2rust_doctrine_dbal::platforms::SQLitePlatform;
use php2rust_doctrine_dbal::{Exception, ServerVersionProvider};

/// `createStub(Connection::class)`.
struct StubConnection;

impl Connection for StubConnection {
    fn prepare<'a>(
        &'a self,
        _sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn Statement>, Exception>> {
        Box::pin(async { Err(Exception::new("RuntimeException", "not configured")) })
    }

    fn query<'a>(
        &'a self,
        _sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async { Err(Exception::new("RuntimeException", "not configured")) })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        PhpString::from(value.to_vec())
    }

    fn exec<'a>(&'a self, _sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async { Ok(0) })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async { Ok(Value::Null) })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async { Ok(()) })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async { Ok(()) })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async { Ok(()) })
    }

    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        Box::pin(async { Ok(String::new()) })
    }

    fn get_native_connection(&self) -> NativeConnection {
        Arc::new(())
    }
}

/// `createMock(Driver::class)`.
struct MockDriver {
    connects: Mutex<Vec<PhpArray>>,
    connection: Arc<dyn Connection>,
}

impl Driver for MockDriver {
    fn class(&self) -> &'static str {
        "Mock_Driver"
    }

    fn connect<'a>(
        &'a self,
        params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn Connection>, Exception>> {
        self.connects.lock().unwrap().push(params.clone());
        Box::pin(async move { Ok(Arc::clone(&self.connection)) })
    }

    fn get_database_platform<'a>(
        &'a self,
        _version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async { Ok(Box::new(SQLitePlatform::new()) as PlatformBox) })
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(SQLiteExceptionConverter)
    }
}

/// `new class ($driver) extends AbstractDriverMiddleware {}`.
fn create_middleware(driver: Arc<MockDriver>) -> AbstractDriverMiddleware {
    AbstractDriverMiddleware::new(driver)
}

#[tokio::test]
async fn testConnect() {
    let connection: Arc<dyn Connection> = Arc::new(StubConnection);
    let driver = Arc::new(MockDriver {
        connects: Mutex::new(Vec::new()),
        connection: Arc::clone(&connection),
    });

    let actual = create_middleware(driver.clone())
        .connect(&php_array!["host" => "localhost"])
        .await
        .unwrap();

    assert!(Arc::ptr_eq(&actual, &connection));
    assert_eq!(
        *driver.connects.lock().unwrap(),
        [php_array!["host" => "localhost"]]
    );
}
