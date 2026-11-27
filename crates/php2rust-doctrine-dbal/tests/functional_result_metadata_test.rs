//! Port of tests/Functional/ResultMetadataTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory
//! under the oracle's PHP 8.4.27. Names and assertions are PHPUnit's.
//! `testColumnCountAfterFree`, which upstream skips on `pdo_sqlite`, is
//! not here (parity marks it n/a).
//!
//! Upstream: doctrine/dbal tests/Functional/ResultMetadataTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, Exception, ParameterTypes, Result};
use schema_manager_support::{column, drop_and_create_table, platform, primary_key, sqlite};

async fn set_up() -> Connection {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("result_metadata_table", None)
        .set_columns(vec![column("test_int", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["test_int"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let mut data = PhpArray::new();
    data.insert("test_int", Value::Int(1));
    connection
        .insert("result_metadata_table", &data, &ParameterTypes::new())
        .await
        .unwrap();

    connection
}

async fn execute_query<'c>(connection: &'c Connection, sql: &str) -> Result<'c> {
    connection
        .execute_query(sql, &PhpArray::new(), &ParameterTypes::new())
        .await
        .unwrap()
}

/// `strtolower($result->getColumnName($index))`.
fn column_name(result: &Result<'_>, index: i64) -> String {
    result
        .get_column_name(index)
        .unwrap()
        .to_string_lossy()
        .to_lowercase()
}

fn expect_invalid_column_index(result: std::result::Result<php_value::PhpString, Exception>) {
    let e = result.unwrap_err();
    assert!(e.is_instance_of(Exception::INVALID_COLUMN_INDEX), "{e}");
}

#[tokio::test]
async fn testColumnNameWithResults() {
    let connection = set_up().await;
    let sql = "SELECT test_int, test_int AS alternate_name FROM result_metadata_table";

    let result = execute_query(&connection, sql).await;

    assert_eq!(2, result.column_count().unwrap());

    // Depending on the platform, field names might have different case than in the SQL
    // query (for instance, Oracle turns unquoted identifiers into upper case).
    assert_eq!("test_int", column_name(&result, 0));
    assert_eq!("alternate_name", column_name(&result, 1));
}

/// `#[TestWith([2])]`, `#[TestWith([-1])]`.
#[tokio::test]
async fn testColumnNameWithInvalidIndex() {
    for index in [2, -1] {
        let connection = set_up().await;
        let sql = "SELECT test_int, test_int AS alternate_name FROM result_metadata_table";

        let mut result = execute_query(&connection, sql).await;

        // Consume the result set to avoid issues with unprocessed buffer between tests
        result.fetch_all_associative().unwrap();

        expect_invalid_column_index(result.get_column_name(index));
    }
}

/// `pdo_sqlite` on PHP 8.4.27 (8.4.5 on reports the failure).
#[tokio::test]
async fn testColumnNameAfterFree() {
    let connection = set_up().await;
    let mut result = execute_query(
        &connection,
        &platform(&connection).await.get_dummy_select_sql("1"),
    )
    .await;
    result.free();

    expect_invalid_column_index(result.get_column_name(0));
}

#[tokio::test]
async fn testColumnNameWithoutResults() {
    let connection = set_up().await;
    let sql = "SELECT test_int, test_int AS alternate_name FROM result_metadata_table WHERE 1 = 0";

    let result = execute_query(&connection, sql).await;

    assert_eq!(2, result.column_count().unwrap());

    assert_eq!("test_int", column_name(&result, 0));
    assert_eq!("alternate_name", column_name(&result, 1));
}
