//! Port of tests/Functional/BlobTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Only the tests that use no stream resource are here: `php_value` has
//! no resources, and `BlobType::convertToPHPValue()` gives one (parity
//! keeps the others pending).
//!
//! Upstream: doctrine/dbal tests/Functional/BlobTest.php
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
use php2rust_doctrine_dbal::{Connection, ParameterType, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, primary_key, sqlite};

async fn set_up() -> Connection {
    let connection = sqlite();
    // pdo_sqlite, not pdo_oci.
    let table = Table::editor()
        .set_unquoted_name("blob_table", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("clobcolumn", Types::TEXT)
                .set_not_null(false)
                .create()
                .unwrap(),
            column("blobcolumn", Types::BLOB)
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    connection
}

/// `[ParameterType::INTEGER, ParameterType::STRING, ParameterType::LARGE_OBJECT]`.
fn insert_types() -> ParameterTypes {
    [
        ParameterType::Integer,
        ParameterType::String,
        ParameterType::LargeObject,
    ]
    .into_iter()
    .zip(0_i64..)
    .map(|(type_, index)| (index.into(), type_.into()))
    .collect()
}

fn row(id: i64, clob: Value, blob: Value) -> PhpArray {
    let mut data = PhpArray::new();
    data.insert("id", Value::Int(id));
    data.insert("clobcolumn", clob);
    data.insert("blobcolumn", blob);
    data
}

/// `fetchRow()`.
async fn fetch_row(connection: &Connection) -> PhpArray {
    let mut rows = connection
        .fetch_all_numeric(
            "SELECT clobcolumn, blobcolumn FROM blob_table",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();

    assert_eq!(rows.len(), 1);

    rows.remove(0)
}

#[tokio::test]
async fn testInsert() {
    let connection = set_up().await;
    let ret = connection
        .insert(
            "blob_table",
            &row(1, Value::string("test"), Value::string("test")),
            &insert_types(),
        )
        .await
        .unwrap();

    assert_eq!(1, ret);
}

#[tokio::test]
async fn testInsertNull() {
    let connection = set_up().await;
    let ret = connection
        .insert(
            "blob_table",
            &row(1, Value::Null, Value::Null),
            &insert_types(),
        )
        .await
        .unwrap();

    assert_eq!(1, ret);

    let row = fetch_row(&connection).await;
    assert_eq!(row.len(), 2);

    assert!(row.get(&0_i64.into()).unwrap().is_null());
    assert!(row.get(&1_i64.into()).unwrap().is_null());
}
