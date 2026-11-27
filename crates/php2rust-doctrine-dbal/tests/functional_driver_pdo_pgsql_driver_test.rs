//! Port of tests/Functional/Driver/PDO/PgSQL/DriverTest.php and the tests
//! it inherits from tests/Functional/Driver/AbstractPostgreSQLDriverTestCase.php
//! and AbstractDriverTestCase.php, on a live `PostgreSQL` server (the
//! `pdo_pgsql` cargo feature and `DBAL_TEST_PGSQL_URL`; without the
//! variable each test passes with a note).
//!
//! Upstream: doctrine/dbal tests/Functional/Driver/PDO/PgSQL/DriverTest.php
//! Upstream: doctrine/dbal tests/Functional/Driver/AbstractPostgreSQLDriverTestCase.php
//! Upstream: doctrine/dbal tests/Functional/Driver/AbstractDriverTestCase.php
#![cfg(feature = "pdo_pgsql")]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod pgsql_support;

use php_value::{Key, Value};
use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::driver::{Driver as _, pdo_pgsql::Driver};

/// `createDriver()`.
fn create_driver() -> Driver {
    Driver::new()
}

#[tokio::test]
async fn testConnectsWithApplicationNameParameter() {
    let Some(live) = pgsql_support::pgsql("testConnectsWithApplicationNameParameter").await else {
        return;
    };
    let mut parameters = live.get_params().clone();
    parameters.insert("application_name", Value::string("doctrine"));

    let connection = create_driver().connect(&parameters).await.unwrap();

    // required to identify the record in the results uniquely
    let hash = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let sql = format!("SELECT * FROM pg_stat_activity WHERE {hash} = {hash}");
    let records = connection
        .query(&sql)
        .await
        .unwrap()
        .fetch_all_associative()
        .unwrap();

    for record in &records {
        // The query column is named "current_query" on PostgreSQL < 9.2
        let query_column_name = if record.contains_key(&Key::from("current_query")) {
            "current_query"
        } else {
            "query"
        };

        if record
            .get(&Key::from(query_column_name))
            .is_some_and(|query| query.identical(&Value::string(sql.as_str())))
        {
            assert!(
                record
                    .get(&Key::from("application_name"))
                    .is_some_and(|name| name.identical(&Value::string("doctrine")))
            );
            return;
        }
    }

    panic!("Query result does not contain a record where column \"query\" equals \"{sql}\".");
}

#[tokio::test]
async fn testConnectsWithoutDatabaseNameParameter() {
    let Some(connection) = pgsql_support::pgsql("testConnectsWithoutDatabaseNameParameter").await
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
        pgsql_support::pgsql("testReturnsDatabaseNameWithoutDatabaseNameParameter").await
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

    // getDatabaseNameForConnectionWithoutDatabaseNameParameter(): 'postgres'.
    assert!(
        connection
            .get_database()
            .await
            .unwrap()
            .is_some_and(|name| name.identical(&Value::string("postgres")))
    );
}

#[tokio::test]
async fn testProvidesAccessToTheNativeConnection() {
    let Some(connection) = pgsql_support::pgsql("testProvidesAccessToTheNativeConnection").await
    else {
        return;
    };
    let native_connection = connection.get_native_connection().await.unwrap();

    // An object: the driver's connection (PHP's PDO), not the unit
    // stand-in of a driver without one.
    assert!(!native_connection.is::<()>());
}
