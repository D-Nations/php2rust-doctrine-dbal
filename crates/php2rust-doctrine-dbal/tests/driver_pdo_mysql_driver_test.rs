//! Port of tests/Driver/PDO/MySQL/DriverTest.php (the `pdo_mysql` cargo
//! feature). The parameters are refused before the driver connects.
//!
//! Upstream: doctrine/dbal tests/Driver/PDO/MySQL/DriverTest.php
#![cfg(feature = "pdo_mysql")]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::driver::{Driver as _, pdo_mysql::Driver};

async fn connect_error(key: &str) -> Exception {
    let mut params = PhpArray::new();
    params.insert(key, Value::Bool(false));
    Driver::new().connect(&params).await.err().unwrap()
}

#[tokio::test]
async fn testUserIsFalse() {
    let e = connect_error("user").await;
    assert!(
        e.is_instance_of(Exception::PDO_INVALID_CONFIGURATION),
        "{e}"
    );
    assert_eq!(
        "The user configuration parameter is expected to be either a string or null, got bool.",
        e.message().to_string_lossy()
    );
}

#[tokio::test]
async fn testPasswordIsFalse() {
    let e = connect_error("password").await;
    assert!(
        e.is_instance_of(Exception::PDO_INVALID_CONFIGURATION),
        "{e}"
    );
    assert_eq!(
        "The password configuration parameter is expected to be either a string or null, got bool.",
        e.message().to_string_lossy()
    );
}
