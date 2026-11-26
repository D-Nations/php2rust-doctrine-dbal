//! Port of tests/Functional/Platform/ColumnTest/SQLite.php and the tests
//! it inherits from tests/Functional/Platform/AbstractColumnTestCase.php,
//! at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's; a data
//! provider's cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/ColumnTest/SQLite.php
//! Upstream: doctrine/dbal tests/Functional/Platform/AbstractColumnTestCase.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{Key, PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::schema::{Column, ColumnEditor, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{ParameterType, ParameterTypes};
use schema_manager_support::{drop_and_create_table, platform, sqlite};

/// `string1Provider()`.
const STRING1: [(&str, &str); 2] = [("ansi", "Z"), ("unicode", "Я")];

/// `string8Provider()`.
const STRING8: [(&str, &str); 2] = [("ansi", "Doctrine"), ("unicode", "Доктрина")];

fn bytes(value: &[u8]) -> Value {
    Value::String(PhpString::from(value.to_vec()))
}

/// `assertColumn($editor, $value, $bindType)`.
async fn assert_column(editor: ColumnEditor, value: Value, bind_type: ParameterType) {
    let column = editor.set_unquoted_name("val").create().unwrap();

    let table = Table::editor()
        .set_unquoted_name("column_test", None)
        .set_columns(vec![column.clone()])
        .create()
        .unwrap();

    let connection = sqlite();
    drop_and_create_table(&connection, &table).await;

    let mut data = PhpArray::new();
    data.insert("val", value.clone());
    let mut types = ParameterTypes::new();
    types.insert(Key::Int(0), bind_type.into());
    assert_eq!(
        1,
        connection
            .insert("column_test", &data, &types)
            .await
            .unwrap()
    );

    let fetched = connection
        .fetch_one(
            "SELECT val FROM column_test",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
        .unwrap_or(Value::Bool(false));
    let converted = column
        .get_type()
        .unwrap()
        .convert_to_php_value(&fetched, platform(&connection).await.as_dyn())
        .unwrap();
    assert!(value.identical(&converted), "{value:?} {converted:?}");
}

fn string() -> ColumnEditor {
    Column::editor().set_type_name(Types::STRING)
}

fn binary() -> ColumnEditor {
    Column::editor().set_type_name(Types::BINARY)
}

#[tokio::test]
async fn testVariableLengthStringNoLength() {
    assert_column(string(), Value::string("Test"), ParameterType::String).await;
}

#[tokio::test]
async fn testVariableLengthStringWithLength() {
    for (_, value) in STRING8 {
        assert_column(
            string().set_length(Some(8)),
            Value::string(value),
            ParameterType::String,
        )
        .await;
    }
}

#[tokio::test]
async fn testFixedLengthStringNoLength() {
    for (_, value) in STRING1 {
        assert_column(
            string().set_fixed(true),
            Value::string(value),
            ParameterType::String,
        )
        .await;
    }
}

#[tokio::test]
async fn testFixedLengthStringWithLength() {
    for (_, value) in STRING8 {
        assert_column(
            string().set_fixed(true).set_length(Some(8)),
            Value::string(value),
            ParameterType::String,
        )
        .await;
    }
}

#[tokio::test]
async fn testVariableLengthBinaryNoLength() {
    assert_column(binary(), bytes(b"\x00\x01\x02\x03"), ParameterType::Binary).await;
}

#[tokio::test]
async fn testVariableLengthBinaryWithLength() {
    assert_column(
        binary().set_length(Some(8)),
        bytes(b"\xCE\xC6\x6B\xDD\x9F\xD8\x07\xB4"),
        ParameterType::Binary,
    )
    .await;
}

#[tokio::test]
async fn testFixedLengthBinaryNoLength() {
    assert_column(
        binary().set_fixed(true),
        bytes(b"\xFF"),
        ParameterType::Binary,
    )
    .await;
}

#[tokio::test]
async fn testFixedLengthBinaryWithLength() {
    assert_column(
        binary().set_fixed(true).set_length(Some(8)),
        bytes(b"\xA0\x0A\x7B\x0E\xA4\x60\x78\xD8"),
        ParameterType::Binary,
    )
    .await;
}
