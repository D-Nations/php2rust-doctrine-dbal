//! Port of tests/Driver/Middleware/AbstractConnectionMiddlewareTest.php.
//! PHPUnit's mock connection is a double that records its calls and
//! returns the statement, result or value the test gives it;
//! `assertSame()` on an object compares the addresses.
//!
//! Upstream: doctrine/dbal tests/Driver/Middleware/AbstractConnectionMiddlewareTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{Key, PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::driver::middleware::AbstractConnectionMiddleware;
use php2rust_doctrine_dbal::driver::{
    BoxFuture, Connection, NativeConnection, Result as DriverResult, Statement,
};
use php2rust_doctrine_dbal::{Exception, ParameterType};

/// `createStub(Statement::class)`.
struct StubStatement;

impl Statement for StubStatement {
    fn bind_value(
        &mut self,
        _param: &Key,
        _value: Value,
        _type: ParameterType,
    ) -> Result<(), Exception> {
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async { Ok(Box::new(StubResult) as Box<dyn DriverResult>) })
    }
}

/// `createStub(Result::class)`.
struct StubResult;

impl DriverResult for StubResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(None)
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(None)
    }

    fn row_count(&self) -> Result<i64, Exception> {
        Ok(0)
    }

    fn column_count(&self) -> Result<usize, Exception> {
        Ok(0)
    }

    fn free(&mut self) {}

    fn class(&self) -> &'static str {
        "Mock_Result"
    }
}

/// `createMock(Connection::class)`: the calls it got, and what it returns.
#[derive(Default)]
struct MockConnection {
    calls: Mutex<Vec<String>>,
    statement: Mutex<Option<Box<dyn Statement>>>,
    result: Mutex<Option<Box<dyn DriverResult>>>,
    exec: i64,
    server_version: String,
    native_connection: Option<NativeConnection>,
}

impl MockConnection {
    fn call(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Connection for MockConnection {
    fn prepare<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<Box<dyn Statement>, Exception>> {
        self.call(format!("prepare {sql}"));
        let statement = self.statement.lock().unwrap().take().unwrap();
        Box::pin(async move { Ok(statement) })
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        self.call(format!("query {sql}"));
        let result = self.result.lock().unwrap().take().unwrap();
        Box::pin(async move { Ok(result) })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        PhpString::from(value.to_vec())
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        self.call(format!("exec {sql}"));
        Box::pin(async move { Ok(self.exec) })
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
        self.call("getServerVersion".to_owned());
        Box::pin(async move { Ok(self.server_version.clone()) })
    }

    fn get_native_connection(&self) -> NativeConnection {
        self.native_connection.clone().unwrap()
    }
}

/// `new class ($connection) extends AbstractConnectionMiddleware {}`.
fn create_middleware(connection: Arc<MockConnection>) -> AbstractConnectionMiddleware {
    AbstractConnectionMiddleware::new(connection)
}

/// The address of a boxed object.
fn address<T: ?Sized>(object: &T) -> *const () {
    std::ptr::from_ref(object).cast::<()>()
}

#[tokio::test]
async fn testPrepare() {
    let statement: Box<dyn Statement> = Box::new(StubStatement);
    let expected = address(&*statement);
    let connection = Arc::new(MockConnection {
        statement: Mutex::new(Some(statement)),
        ..MockConnection::default()
    });

    let actual = create_middleware(connection.clone())
        .prepare("SELECT 1")
        .await
        .unwrap();

    assert_eq!(address(&*actual), expected);
    assert_eq!(connection.calls(), ["prepare SELECT 1"]);
}

#[tokio::test]
async fn testQuery() {
    let result: Box<dyn DriverResult> = Box::new(StubResult);
    let expected = address(&*result);
    let connection = Arc::new(MockConnection {
        result: Mutex::new(Some(result)),
        ..MockConnection::default()
    });

    let actual = create_middleware(connection.clone())
        .query("SELECT 1")
        .await
        .unwrap();

    assert_eq!(address(&*actual), expected);
    assert_eq!(connection.calls(), ["query SELECT 1"]);
}

#[tokio::test]
async fn testExec() {
    let connection = Arc::new(MockConnection {
        exec: 42,
        ..MockConnection::default()
    });

    assert_eq!(
        create_middleware(connection.clone())
            .exec("UPDATE foo SET bar='baz' WHERE some_field > 0")
            .await
            .unwrap(),
        42
    );
    assert_eq!(
        connection.calls(),
        ["exec UPDATE foo SET bar='baz' WHERE some_field > 0"]
    );
}

#[tokio::test]
async fn testGetServerVersion() {
    let connection = Arc::new(MockConnection {
        server_version: "1.2.3".to_owned(),
        ..MockConnection::default()
    });

    assert_eq!(
        create_middleware(connection.clone())
            .get_server_version()
            .await
            .unwrap(),
        "1.2.3"
    );
    assert_eq!(connection.calls(), ["getServerVersion"]);
}

#[test]
fn testGetNativeConnection() {
    /// `new class () {}`.
    struct NativeConnectionObject;

    let native_connection: NativeConnection = Arc::new(NativeConnectionObject);
    let connection = Arc::new(MockConnection {
        native_connection: Some(native_connection.clone()),
        ..MockConnection::default()
    });

    let actual = create_middleware(connection).get_native_connection();

    assert!(Arc::ptr_eq(&actual, &native_connection));
    assert!(actual.downcast_ref::<NativeConnectionObject>().is_some());
}
