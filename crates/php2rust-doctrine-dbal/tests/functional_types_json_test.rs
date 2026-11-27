//! Port of tests/Functional/Types/JsonTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! `pdo_sqlite` fetches text, so the `is_resource()` branch has nothing
//! to read.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/JsonTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{Connection, ParameterType, ParameterTypes, WrapperParameterType};
use schema_manager_support::{column, drop_and_create_table, platform, primary_key, sqlite};

async fn set_up() -> Connection {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("json_test_table", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("val", Types::JSON).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    connection
}

fn array(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    array
}

async fn insert(connection: &Connection, id: i64, value: &PhpArray) {
    let mut data = PhpArray::new();
    data.insert("id", Value::Int(id));
    data.insert("val", Value::Array(value.clone()));
    let types: ParameterTypes = [
        (0_i64.into(), ParameterType::Integer.into()),
        (
            1_i64.into(),
            WrapperParameterType::from(types::get_type(Types::JSON).unwrap()),
        ),
    ]
    .into_iter()
    .collect();
    let result = connection
        .insert("json_test_table", &data, &types)
        .await
        .unwrap();

    assert_eq!(1, result);
}

async fn select(connection: &Connection, id: i64) -> PhpArray {
    let value = connection
        .fetch_one(
            "SELECT val FROM json_test_table WHERE id = ?",
            &PhpArray::list([Value::Int(id)]),
            &[(0_i64.into(), ParameterType::Integer.into())]
                .into_iter()
                .collect(),
        )
        .await
        .unwrap()
        .unwrap_or(Value::Bool(false));

    assert!(matches!(value, Value::String(_)), "{value:?}");

    let value = types::get_type(Types::JSON)
        .unwrap()
        .convert_to_php_value(&value, platform(connection).await)
        .unwrap();

    let Value::Array(value) = value else {
        panic!("{value:?}");
    };
    value
}

#[tokio::test]
async fn testInsertAndSelect() {
    let connection = set_up().await;
    let id1 = 1;
    let id2 = 2;

    let mut value1 = array([
        ("firstKey", Value::string("firstVal")),
        ("secondKey", Value::string("secondVal")),
        (
            "nestedKey",
            Value::Array(array([
                ("nestedKey1", Value::string("nestedVal1")),
                ("nestedKey2", Value::Int(2)),
            ])),
        ),
    ]);

    // json_decode('{"key1":"Val1","key2":2,"key3":"Val3"}', true)
    let mut value2 = array([
        ("key1", Value::string("Val1")),
        ("key2", Value::Int(2)),
        ("key3", Value::string("Val3")),
    ]);

    insert(&connection, id1, &value1).await;
    insert(&connection, id2, &value2).await;

    let mut res1 = select(&connection, id1).await;
    let mut res2 = select(&connection, id2).await;

    // The returned arrays are not guaranteed to be in the same order so sort them
    value1.ksort();
    value2.ksort();
    res1.ksort();
    res2.ksort();

    assert!(Value::Array(value1).identical(&Value::Array(res1)));
    assert!(Value::Array(value2).identical(&Value::Array(res2)));
}
