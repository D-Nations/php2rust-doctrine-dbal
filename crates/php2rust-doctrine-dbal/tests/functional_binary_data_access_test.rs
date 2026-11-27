//! Port of tests/Functional/BinaryDataAccessTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's. `pdo_sqlite` fetches binary values
//! as strings, so upstream's `is_resource()` branches have nothing to read.
//!
//! Upstream: doctrine/dbal tests/Functional/BinaryDataAccessTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{
    ArrayParameterType, Connection, ParameterType, ParameterTypes, WrapperParameterType,
};
use schema_manager_support::{column, drop_and_create_table, primary_key, sqlite};

/// `hex2bin($hex)`.
fn hex2bin(hex: &str) -> Value {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    Value::string(PhpString::from(bytes))
}

fn types<K: Into<Key>>(
    pairs: impl IntoIterator<Item = (K, WrapperParameterType)>,
) -> ParameterTypes {
    pairs
        .into_iter()
        .map(|(key, type_)| (key.into(), type_))
        .collect()
}

/// `[1 => ParameterType::BINARY]`.
fn binary_at_1() -> ParameterTypes {
    types([(1_i64, ParameterType::Binary.into())])
}

/// `[ParameterType::STRING, Types::BINARY]`.
fn string_and_binary() -> ParameterTypes {
    types([
        (0_i64, ParameterType::String.into()),
        (1_i64, WrapperParameterType::from(Types::BINARY)),
    ])
}

fn params() -> PhpArray {
    PhpArray::list([Value::Int(1), hex2bin("C0DEF00D")])
}

async fn set_up() -> Connection {
    let connection = sqlite();
    // pdo_sqlite, not pdo_oci.
    let table = Table::editor()
        .set_unquoted_name("binary_fetch_table", None)
        .set_columns(vec![
            column("test_int", Types::INTEGER).create().unwrap(),
            column("test_binary", Types::BINARY)
                .set_not_null(false)
                .set_length(Some(4))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["test_int"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    insert(&connection, 1, hex2bin("C0DEF00D")).await;

    connection
}

async fn insert(connection: &Connection, test_int: i64, test_binary: Value) {
    let mut data = PhpArray::new();
    data.insert("test_int", Value::Int(test_int));
    data.insert("test_binary", test_binary);
    connection
        .insert(
            "binary_fetch_table",
            &data,
            &types([("test_binary", ParameterType::Binary.into())]),
        )
        .await
        .unwrap();
}

/// `array_change_key_case($row, CASE_LOWER)`.
fn lower(row: &PhpArray) -> PhpArray {
    let mut lower = PhpArray::new();
    for (key, value) in row.iter() {
        let key = match key {
            Key::Str(name) => Key::from_bytes(&name.as_bytes().to_ascii_lowercase()),
            Key::Int(_) => key.clone(),
        };
        lower.insert(key, value.clone());
    }
    lower
}

fn get(row: &PhpArray, key: impl Into<Key>) -> Value {
    row.get(&key.into()).cloned().unwrap_or(Value::Null)
}

fn keys(row: &PhpArray) -> Value {
    Value::Array(PhpArray::list(row.iter().map(|(key, _)| match key {
        Key::Str(name) => Value::string(name.clone()),
        Key::Int(i) => Value::Int(*i),
    })))
}

fn assert_equals(expected: &Value, actual: &Value) {
    assert!(
        phpunit_equals(expected, actual),
        "expected {expected:?}, got {actual:?}"
    );
}

const SELECT: &str =
    "SELECT test_int, test_binary FROM binary_fetch_table WHERE test_int = ? AND test_binary = ?";

#[tokio::test]
async fn testPrepareWithBindValue() {
    let connection = set_up().await;
    let mut stmt = connection.prepare(SELECT).await.unwrap();

    stmt.bind_string(1, Value::Int(1)).unwrap();
    stmt.bind_value(2, hex2bin("C0DEF00D"), ParameterType::Binary)
        .unwrap();

    let row = stmt
        .execute_query()
        .await
        .unwrap()
        .fetch_associative()
        .unwrap();

    let row = lower(&row.unwrap());
    assert_equals(
        &Value::Array(PhpArray::list([
            Value::string("test_int"),
            Value::string("test_binary"),
        ])),
        &keys(&row),
    );
    assert_equals(&Value::Int(1), &get(&row, "test_int"));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, "test_binary"));
}

#[tokio::test]
async fn testPrepareWithFetchAllAssociative() {
    let connection = set_up().await;
    let mut stmt = connection.prepare(SELECT).await.unwrap();

    stmt.bind_string(1, Value::Int(1)).unwrap();
    stmt.bind_value(2, hex2bin("C0DEF00D"), ParameterType::Binary)
        .unwrap();

    let rows = stmt
        .execute_query()
        .await
        .unwrap()
        .fetch_all_associative()
        .unwrap();
    let row = lower(&rows[0]);

    assert_equals(
        &Value::Array(PhpArray::list([
            Value::string("test_int"),
            Value::string("test_binary"),
        ])),
        &keys(&row),
    );
    assert_equals(&Value::Int(1), &get(&row, "test_int"));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, "test_binary"));
}

#[tokio::test]
async fn testPrepareWithFetchOne() {
    let connection = set_up().await;
    let mut stmt = connection
        .prepare("SELECT test_int FROM binary_fetch_table WHERE test_int = ? AND test_binary = ?")
        .await
        .unwrap();

    stmt.bind_string(1, Value::Int(1)).unwrap();
    stmt.bind_value(2, hex2bin("C0DEF00D"), ParameterType::Binary)
        .unwrap();

    let column = stmt.execute_query().await.unwrap().fetch_one().unwrap();
    assert_equals(&Value::Int(1), &column.unwrap());
}

#[tokio::test]
async fn testFetchAllAssociative() {
    let connection = set_up().await;
    let data = connection
        .fetch_all_associative(SELECT, &params(), &binary_at_1())
        .await
        .unwrap();

    assert_eq!(data.len(), 1);

    let row = &data[0];
    assert_eq!(row.len(), 2);

    let row = lower(row);
    assert_equals(&Value::Int(1), &get(&row, "test_int"));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, "test_binary"));
}

#[tokio::test]
async fn testFetchAllWithTypes() {
    let connection = set_up().await;
    let data = connection
        .fetch_all_associative(SELECT, &params(), &string_and_binary())
        .await
        .unwrap();

    assert_eq!(data.len(), 1);

    let row = &data[0];
    assert_eq!(row.len(), 2);

    let row = lower(row);
    assert_equals(&Value::Int(1), &get(&row, "test_int"));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, "test_binary"));
}

#[tokio::test]
async fn testFetchAssociative() {
    let connection = set_up().await;
    let row = connection
        .fetch_associative(SELECT, &params(), &binary_at_1())
        .await
        .unwrap();

    let row = lower(&row.unwrap());

    assert_equals(&Value::Int(1), &get(&row, "test_int"));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, "test_binary"));
}

#[tokio::test]
async fn testFetchAssocWithTypes() {
    let connection = set_up().await;
    let row = connection
        .fetch_associative(SELECT, &params(), &string_and_binary())
        .await
        .unwrap();

    let row = lower(&row.unwrap());

    assert_equals(&Value::Int(1), &get(&row, "test_int"));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, "test_binary"));
}

#[tokio::test]
async fn testFetchArray() {
    let connection = set_up().await;
    let row = connection
        .fetch_numeric(SELECT, &params(), &binary_at_1())
        .await
        .unwrap()
        .unwrap();

    assert_equals(&Value::Int(1), &get(&row, 0_i64));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, 1_i64));
}

#[tokio::test]
async fn testFetchArrayWithTypes() {
    let connection = set_up().await;
    let row = connection
        .fetch_numeric(SELECT, &params(), &string_and_binary())
        .await
        .unwrap();

    let row = lower(&row.unwrap());

    assert_equals(&Value::Int(1), &get(&row, 0_i64));
    assert_equals(&hex2bin("C0DEF00D"), &get(&row, 1_i64));
}

#[tokio::test]
async fn testFetchColumn() {
    let connection = set_up().await;
    let test_int = connection
        .fetch_one(
            "SELECT test_int FROM binary_fetch_table WHERE test_int = ? AND test_binary = ?",
            &params(),
            &binary_at_1(),
        )
        .await
        .unwrap();

    assert_equals(&Value::Int(1), &test_int.unwrap());

    let test_binary = connection
        .fetch_one(
            "SELECT test_binary FROM binary_fetch_table WHERE test_int = ? AND test_binary = ?",
            &params(),
            &binary_at_1(),
        )
        .await
        .unwrap();

    assert_equals(&hex2bin("C0DEF00D"), &test_binary.unwrap());
}

#[tokio::test]
async fn testFetchOneWithTypes() {
    let connection = set_up().await;
    let column = connection
        .fetch_one(
            "SELECT test_binary FROM binary_fetch_table WHERE test_int = ? AND test_binary = ?",
            &params(),
            &string_and_binary(),
        )
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(column, Value::String(_)), "{column:?}");
    assert_equals(&hex2bin("C0DEF00D"), &column);
}

#[tokio::test]
async fn testNativeArrayListSupport() {
    let connection = set_up().await;
    let binary_values: Vec<Value> = [
        "A0AEFA", "1F43BA", "8C9D2A", "72E8AA", "5B6F9A", "DAB24A", "3E71CA", "F0D6EA", "6A8B5A",
        "C582FA",
    ]
    .into_iter()
    .map(hex2bin)
    .collect();

    for i in 100..110_i64 {
        insert(
            &connection,
            i,
            binary_values[usize::try_from(i - 100).unwrap()].clone(),
        )
        .await;
    }

    let expected = Value::Array(PhpArray::list(
        [100, 101, 102, 103, 104].map(|n| Value::Array(PhpArray::list([Value::Int(n)]))),
    ));
    let first_five = || {
        PhpArray::list([Value::Array(PhpArray::list(
            binary_values[..5].iter().cloned(),
        ))])
    };

    let data = connection
        .execute_query(
            "SELECT test_int FROM binary_fetch_table WHERE test_int IN (?)",
            &PhpArray::list([Value::Array(PhpArray::list(
                [100, 101, 102, 103, 104].map(Value::Int),
            ))]),
            &types([(0_i64, ArrayParameterType::Integer.into())]),
        )
        .await
        .unwrap()
        .fetch_all_numeric()
        .unwrap();
    assert_eq!(data.len(), 5);
    assert_equals(
        &expected,
        &Value::Array(PhpArray::list(data.into_iter().map(Value::Array))),
    );

    let data = connection
        .execute_query(
            "SELECT test_int FROM binary_fetch_table WHERE test_binary IN (?)",
            &first_five(),
            &types([(0_i64, ArrayParameterType::Binary.into())]),
        )
        .await
        .unwrap()
        .fetch_all_numeric()
        .unwrap();
    assert_eq!(data.len(), 5);
    assert_equals(
        &expected,
        &Value::Array(PhpArray::list(data.into_iter().map(Value::Array))),
    );

    let data = connection
        .execute_query(
            "SELECT test_binary FROM binary_fetch_table WHERE test_binary IN (?)",
            &first_five(),
            &types([(0_i64, ArrayParameterType::Binary.into())]),
        )
        .await
        .unwrap()
        .fetch_first_column()
        .unwrap();
    assert_eq!(data.len(), 5);
    assert_equals(
        &Value::Array(PhpArray::list(binary_values[..5].iter().cloned())),
        &Value::Array(PhpArray::list(data)),
    );
}
