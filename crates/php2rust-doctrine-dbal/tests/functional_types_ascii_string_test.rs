//! Port of tests/Functional/Types/AsciiStringTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/AsciiStringTest.php
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
    let table = Table::editor()
        .set_unquoted_name("ascii_table", None)
        .set_columns(vec![
            column("id", Types::ASCII_STRING)
                .set_length(Some(3))
                .set_fixed(true)
                .create()
                .unwrap(),
            column("val", Types::ASCII_STRING)
                .set_length(Some(4))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    connection
}

fn ascii_types(count: i64) -> ParameterTypes {
    (0..count)
        .map(|index| (index.into(), ParameterType::Ascii.into()))
        .collect()
}

async fn insert(connection: &Connection, id: &str, value: &str) {
    let mut data = PhpArray::new();
    data.insert("id", Value::string(id));
    data.insert("val", Value::string(value));
    let result = connection
        .insert("ascii_table", &data, &ascii_types(2))
        .await
        .unwrap();

    assert_eq!(1, result);
}

async fn select(connection: &Connection, id: &str) -> Value {
    let value = connection
        .fetch_one(
            "SELECT val FROM ascii_table WHERE id = ?",
            &PhpArray::list([Value::string(id)]),
            &ascii_types(1),
        )
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(value, Value::String(_)), "{value:?}");
    value
}

#[tokio::test]
async fn testInsertAndSelect() {
    let connection = set_up().await;
    let (id1, id2) = ("id1", "id2");
    let (value1, value2) = ("val1", "val2");

    insert(&connection, id1, value1).await;
    insert(&connection, id2, value2).await;

    assert!(Value::string(value1).identical(&select(&connection, id1).await));
    assert!(Value::string(value2).identical(&select(&connection, id2).await));
}
