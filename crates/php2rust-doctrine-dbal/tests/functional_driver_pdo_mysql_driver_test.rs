//! Port of tests/Functional/Driver/PDO/MySQL/DriverTest.php and the tests
//! it inherits from tests/Functional/Driver/AbstractDriverTestCase.php, on
//! a live `MySQL` server (the `pdo_mysql` cargo feature and
//! `DBAL_TEST_MYSQL_URL`; without the variable each test passes with a
//! note).
//!
//! Upstream: doctrine/dbal tests/Functional/Driver/PDO/MySQL/DriverTest.php
//! Upstream: doctrine/dbal tests/Functional/Driver/AbstractDriverTestCase.php
#![cfg(feature = "pdo_mysql")]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod mysql_support;

use php_value::Key;
use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::driver::{Driver as _, pdo_mysql::Driver};

/// `createDriver()`.
fn create_driver() -> Driver {
    Driver::new()
}

#[tokio::test]
async fn testConnectsWithoutDatabaseNameParameter() {
    let Some(connection) = mysql_support::mysql("testConnectsWithoutDatabaseNameParameter").await
    else {
        return;
    };
    let mut params = connection.get_params().clone();
    params.remove(&Key::from("dbname"));

    create_driver().connect(&params).await.unwrap();
}

#[tokio::test]
async fn testReturnsDatabaseNameWithoutDatabaseNameParameter() {
    let Some(live) =
        mysql_support::mysql("testReturnsDatabaseNameWithoutDatabaseNameParameter").await
    else {
        return;
    };
    let mut params = live.get_params().clone();
    params.remove(&Key::from("dbname"));

    let connection = Connection::new(
        params,
        live.get_driver().clone(),
        Some(live.get_configuration().clone()),
    );

    // getDatabaseNameForConnectionWithoutDatabaseNameParameter(): null.
    assert_eq!(
        None,
        connection
            .get_database()
            .await
            .unwrap()
            .filter(|v| !v.is_null())
    );
}

#[tokio::test]
async fn testProvidesAccessToTheNativeConnection() {
    let Some(connection) = mysql_support::mysql("testProvidesAccessToTheNativeConnection").await
    else {
        return;
    };
    let native_connection = connection.get_native_connection().await.unwrap();

    // An object: the driver's connection (PHP's PDO).
    assert!(
        native_connection
            .downcast_ref::<tokio::sync::Mutex<php2rust_doctrine_dbal::driver::pdo_mysql::Inner>>()
            .is_some()
    );
}
