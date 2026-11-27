//! Port of tests/Driver/Middleware/AbstractStatementMiddlewareTest.php.
//! PHPUnit's mock statement is a double that counts its `execute()` calls
//! and returns the stub result; `assertSame()` on an object compares the
//! addresses.
//!
//! Upstream: doctrine/dbal tests/Driver/Middleware/AbstractStatementMiddlewareTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::driver::middleware::AbstractStatementMiddleware;
use php2rust_doctrine_dbal::driver::{BoxFuture, Result as DriverResult, Statement};
use php2rust_doctrine_dbal::{Exception, ParameterType};

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

/// `createMock(Statement::class)`.
struct MockStatement {
    execute_calls: Arc<AtomicUsize>,
    result: Option<Box<dyn DriverResult>>,
}

impl Statement for MockStatement {
    fn bind_value(
        &mut self,
        _param: &Key,
        _value: Value,
        _type: ParameterType,
    ) -> Result<(), Exception> {
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        self.execute_calls.fetch_add(1, Ordering::SeqCst);
        let result = self.result.take().unwrap();
        Box::pin(async move { Ok(result) })
    }
}

/// `new class ($statement) extends AbstractStatementMiddleware {}`.
fn create_middleware(statement: MockStatement) -> AbstractStatementMiddleware {
    AbstractStatementMiddleware::new(Box::new(statement))
}

/// The address of a boxed object.
fn address<T: ?Sized>(object: &T) -> *const () {
    std::ptr::from_ref(object).cast::<()>()
}

#[tokio::test]
async fn testExecute() {
    let result: Box<dyn DriverResult> = Box::new(StubResult);
    let expected = address(&*result);
    let calls = Arc::new(AtomicUsize::new(0));
    let statement = MockStatement {
        execute_calls: calls.clone(),
        result: Some(result),
    };

    let actual = create_middleware(statement).execute().await.unwrap();

    assert_eq!(address(&*actual), expected);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
