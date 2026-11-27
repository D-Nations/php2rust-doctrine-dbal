//! Port of tests/Driver/PDO/ExceptionTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. The `PDOException` is an `Exception` of class `PDOException`
//! with its `errorInfo` beside it; `assertSame()` of the previous exception
//! compares its class and message.
//!
//! Upstream: doctrine/dbal tests/Driver/PDO/ExceptionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::driver::pdo_sqlite::{self, PDO_EXCEPTION_CLASS};

const ERROR_CODE: i64 = 666;

const MESSAGE: &str = "PDO Exception";

const SQLSTATE: &str = "HY000";

/// `new PDOException(self::MESSAGE)`.
fn wrapped_exception() -> Exception {
    Exception::new(PDO_EXCEPTION_CLASS, MESSAGE)
}

/// `setUp()`: the PDO exception wrapper under test, with the wrapped PDO
/// exception's `errorInfo` set to `[self::SQLSTATE, self::ERROR_CODE]`.
fn exception() -> Exception {
    pdo_sqlite::exception(
        wrapped_exception(),
        Some((Some(SQLSTATE.to_owned()), Some(ERROR_CODE))),
    )
}

#[test]
fn testReturnsCode() {
    assert_eq!(ERROR_CODE, exception().code());
}

#[test]
fn testReturnsMessage() {
    assert_eq!(MESSAGE, exception().message().to_string_lossy());
}

#[test]
fn testReturnsSQLState() {
    assert_eq!(Some(SQLSTATE), exception().sql_state());
}

#[test]
fn testOriginalExceptionIsInChain() {
    let exception = exception();
    let previous = exception.previous().unwrap();

    assert_eq!(PDO_EXCEPTION_CLASS, previous.class());
    assert_eq!(
        wrapped_exception().message().as_bytes(),
        previous.message().as_bytes()
    );
}
