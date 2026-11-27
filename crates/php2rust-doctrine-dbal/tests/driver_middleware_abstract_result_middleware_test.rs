//! Port of tests/Driver/Middleware/AbstractResultMiddlewareTest.php.
//! PHPUnit's mock result is a double that counts its `fetchAssociative()`
//! calls and returns the row the test gives it.
//!
//! Upstream: doctrine/dbal tests/Driver/Middleware/AbstractResultMiddlewareTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use php_value::{PhpArray, php_array};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::driver::Result as DriverResult;
use php2rust_doctrine_dbal::driver::middleware::AbstractResultMiddleware;

/// `createMock(Result::class)`.
struct MockResult {
    fetch_associative_calls: Arc<AtomicUsize>,
    row: PhpArray,
}

impl DriverResult for MockResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(None)
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        self.fetch_associative_calls.fetch_add(1, Ordering::SeqCst);
        Ok(Some(self.row.clone()))
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

/// `new class ($result) extends AbstractResultMiddleware {}`.
fn create_middleware(result: MockResult) -> AbstractResultMiddleware {
    AbstractResultMiddleware::new(Box::new(result))
}

#[test]
fn testFetchAssociative() {
    let row = php_array!["field" => "value", "another_field" => 42];
    let calls = Arc::new(AtomicUsize::new(0));
    let result = MockResult {
        fetch_associative_calls: calls.clone(),
        row: row.clone(),
    };

    let actual = create_middleware(result).fetch_associative().unwrap();

    assert_eq!(actual, Some(row));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
