//! Port of tests/Functional/Types/JsonObjectTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! `pdo_sqlite` fetches text, so the `is_resource()` branch has nothing
//! to read.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/JsonObjectTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::object::StdClass;
use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{Connection, ParameterType, ParameterTypes, WrapperParameterType};
use schema_manager_support::{column, drop_and_create_table, primary_key, sqlite};

async fn set_up() -> Connection {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("json_test_table", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("val", Types::JSON_OBJECT).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    connection
}

/// A `stdClass` with these properties.
fn std_class(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    let mut properties = PhpArray::new();
    for (key, value) in pairs {
        properties.insert(key, value);
    }
    StdClass::object(properties)
}

async fn insert(connection: &Connection, id: i64, value: &Value) {
    let mut data = PhpArray::new();
    data.insert("id", Value::Int(id));
    data.insert("val", value.clone());
    let types: ParameterTypes = [
        (0_i64.into(), ParameterType::Integer.into()),
        (
            1_i64.into(),
            WrapperParameterType::from(types::get_type(Types::JSON_OBJECT).unwrap()),
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

async fn select(connection: &Connection, id: i64) -> Value {
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

    let value = connection
        .convert_to_php_value(&value, Types::JSON_OBJECT)
        .await
        .unwrap();

    assert!(matches!(value, Value::Object(_)), "{value:?}");
    value
}

fn assert_equals(expected: &Value, actual: &Value) {
    assert!(
        phpunit_equals(expected, actual),
        "expected {expected:?}, got {actual:?}"
    );
}

#[tokio::test]
async fn testInsertAndSelect() {
    let connection = set_up().await;
    let id1 = 1;
    let id2 = 2;

    let value1 = std_class([
        ("firstKey", Value::string("firstVal")),
        ("secondKey", Value::string("secondVal")),
        (
            "nestedKey",
            std_class([
                ("nestedKey1", Value::string("nestedVal1")),
                ("nestedKey2", Value::Int(2)),
            ]),
        ),
    ]);

    // json_decode('{"key1":"Val1","key2":2,"key3":"Val3"}', false)
    let value2 = std_class([
        ("key1", Value::string("Val1")),
        ("key2", Value::Int(2)),
        ("key3", Value::string("Val3")),
    ]);

    insert(&connection, id1, &value1).await;
    insert(&connection, id2, &value2).await;

    let res1 = select(&connection, id1).await;
    let res2 = select(&connection, id2).await;

    assert_equals(&value1, &res1);
    assert_equals(&value2, &res2);
}
