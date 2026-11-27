//! Port of tests/Functional/Driver/PDO/PgSQL/ConnectionTest.php, on a
//! live `PostgreSQL` server (the `pdo_pgsql` cargo feature and
//! `DBAL_TEST_PGSQL_URL`; without the variable the test passes with a
//! note). The data provider's rows run in one test.
//!
//! Upstream: doctrine/dbal tests/Functional/Driver/PDO/PgSQL/ConnectionTest.php
#![cfg(feature = "pdo_pgsql")]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod pgsql_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::{DriverManager, ParameterTypes};

/// `getValidCharsets()`.
fn get_valid_charsets() -> [&'static str; 2] {
    ["UTF8", "LATIN1"]
}

#[tokio::test]
async fn testConnectsWithValidCharsetOption() {
    let Some(live) = pgsql_support::pgsql("testConnectsWithValidCharsetOption").await else {
        return;
    };
    for charset in get_valid_charsets() {
        let mut params = live.get_params().clone();
        params.insert("charset", Value::string(charset));

        let connection =
            DriverManager::get_connection(params, Some(live.get_configuration().clone())).unwrap();

        // assertEquals($charset, ...)
        let encoding = connection
            .fetch_one(
                "SHOW client_encoding",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap();
        assert!(
            encoding.is_some_and(|e| e.identical(&Value::string(charset))),
            "{charset}"
        );
    }
}
