//! Port of tests/Functional/Types/BinaryTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! `random_bytes()` is a seeded generator here: any bytes do.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/BinaryTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{Connection, ParameterType, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, platform, primary_key, sqlite};

async fn set_up() -> Connection {
    let connection = sqlite();
    // pdo_sqlite, not pdo_oci.
    let table = Table::editor()
        .set_unquoted_name("binary_table", None)
        .set_columns(vec![
            column("id", Types::BINARY)
                .set_length(Some(16))
                .set_fixed(true)
                .create()
                .unwrap(),
            column("val", Types::BINARY)
                .set_length(Some(64))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    connection
}

/// `random_bytes($length)`, from a xorshift generator.
fn random_bytes(state: &mut u64, length: usize) -> Value {
    let bytes: Vec<u8> = (0..length)
        .map(|_| {
            *state ^= *state << 13;
            *state ^= *state >> 7;
            *state ^= *state << 17;
            state.to_le_bytes()[0]
        })
        .collect();
    Value::string(PhpString::from(bytes))
}

fn binary_types(count: i64) -> ParameterTypes {
    (0..count)
        .map(|index| (index.into(), ParameterType::Binary.into()))
        .collect()
}

async fn insert(connection: &Connection, id: &Value, value: &Value) {
    let mut data = PhpArray::new();
    data.insert("id", id.clone());
    data.insert("val", value.clone());
    let result = connection
        .insert("binary_table", &data, &binary_types(2))
        .await
        .unwrap();

    assert_eq!(1, result);
}

async fn select(connection: &Connection, id: &Value) -> Value {
    let value = connection
        .fetch_one(
            "SELECT val FROM binary_table WHERE id = ?",
            &PhpArray::list([id.clone()]),
            &binary_types(1),
        )
        .await
        .unwrap()
        .unwrap_or(Value::Bool(false));

    types::get_type("binary")
        .unwrap()
        .convert_to_php_value(&value, platform(connection).await)
        .unwrap()
}

#[tokio::test]
async fn testInsertAndSelect() {
    let connection = set_up().await;
    let mut state = 0x2545_f491_4f6c_dd1d;
    let id1 = random_bytes(&mut state, 16);
    let id2 = random_bytes(&mut state, 16);

    let value1 = random_bytes(&mut state, 64);
    let value2 = random_bytes(&mut state, 64);

    // pdo_sqlite, not ibm_db2.
    insert(&connection, &id1, &value1).await;
    insert(&connection, &id2, &value2).await;

    assert!(value1.identical(&select(&connection, &id1).await));
    assert!(value2.identical(&select(&connection, &id2).await));
}
