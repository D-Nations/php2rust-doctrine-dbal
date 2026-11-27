//! Port of tests/Driver/PDO/SQLite/DriverTest.php with the tests it
//! inherits from tests/Driver/AbstractSQLiteDriverTestCase.php and
//! tests/Driver/AbstractDriverTestCase.php, at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `testThrowsExceptionOnCreatingDatabasePlatformsForInvalidVersion`
//! is skipped upstream for the `SQLite` drivers (parity marks it n/a).
//!
//! Upstream: doctrine/dbal tests/Driver/PDO/SQLite/DriverTest.php
//! Upstream: doctrine/dbal tests/Driver/AbstractSQLiteDriverTestCase.php
//! Upstream: doctrine/dbal tests/Driver/AbstractDriverTestCase.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::driver::Driver as _;
use php2rust_doctrine_dbal::driver::pdo_sqlite::Driver;

/// `createDriver()`.
fn create_driver() -> Driver {
    Driver::new()
}

async fn expect_invalid_configuration(params: PhpArray, message: &str) {
    let driver = create_driver();
    let Err(e) = driver.connect(&params).await else {
        panic!("Failed asserting that exception of type \"InvalidConfiguration\" is thrown.");
    };

    assert!(
        e.is_instance_of(Exception::PDO_INVALID_CONFIGURATION),
        "{e}"
    );
    assert_eq!(message, e.message().to_string_lossy());
}

#[tokio::test]
async fn testUserIsFalse() {
    let mut params = PhpArray::new();
    params.insert("user", Value::Bool(false));

    expect_invalid_configuration(
        params,
        "The user configuration parameter is expected to be either a string or null, got bool.",
    )
    .await;
}

#[tokio::test]
async fn testPasswordIsFalse() {
    let mut params = PhpArray::new();
    params.insert("password", Value::Bool(false));

    expect_invalid_configuration(
        params,
        "The password configuration parameter is expected to be either a string or null, got bool.",
    )
    .await;
}
