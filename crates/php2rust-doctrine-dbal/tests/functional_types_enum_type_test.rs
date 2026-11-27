//! Port of tests/Functional/Types/EnumTypeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! `testIntrospectEnum` and `testIntrospectEnumValues`, which upstream
//! skips on `SQLite`, are not here (parity marks them n/a). A fresh
//! database per test leaves `setUp()`'s drop nothing to do.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/EnumTypeTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{Exception, ParameterTypes, WrapperParameterType};
use schema_manager_support::{column, drop_and_create_table, primary_key, schema_manager, sqlite};

#[tokio::test]
async fn testDeployEnum() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("my_enum_table", None)
        .set_columns(vec![
            column("id", Types::BIGINT).create().unwrap(),
            column("suit", Types::ENUM)
                .set_values(PhpArray::list(
                    ["hearts", "diamonds", "clubs", "spades"].map(Value::string),
                ))
                .set_default_value(Value::string("hearts"))
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let schema_manager = schema_manager(&connection).await;

    let introspected_table = schema_manager
        .introspect_table_by_unquoted_name("my_enum_table", None)
        .await
        .unwrap();

    assert!(
        schema_manager
            .create_comparator()
            .await
            .unwrap()
            .compare_tables(&introspected_table, &table)
            .unwrap()
            .is_empty()
    );

    let mut data = PhpArray::new();
    data.insert("id", Value::Int(1));
    data.insert("suit", Value::string("hearts"));
    let types: ParameterTypes = [(Key::from("suit"), WrapperParameterType::from(Types::ENUM))]
        .into_iter()
        .collect();
    connection
        .insert("my_enum_table", &data, &types)
        .await
        .unwrap();

    let mut data = PhpArray::new();
    data.insert("id", Value::Int(2));
    data.insert("suit", Value::string("diamonds"));
    let types: ParameterTypes = [(
        Key::from("suit"),
        WrapperParameterType::from(types::get_type(Types::ENUM).unwrap()),
    )]
    .into_iter()
    .collect();
    connection
        .insert("my_enum_table", &data, &types)
        .await
        .unwrap();

    let expected = Value::Array(PhpArray::list([
        Value::Array(PhpArray::list([Value::Int(1), Value::string("hearts")])),
        Value::Array(PhpArray::list([Value::Int(2), Value::string("diamonds")])),
    ]));
    let actual = Value::Array(PhpArray::list(
        connection
            .fetch_all_numeric(
                "SELECT id, suit FROM my_enum_table ORDER BY id ASC",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .into_iter()
            .map(Value::Array),
    ));
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
}

#[tokio::test]
async fn testDeployEmptyEnum() {
    let connection = sqlite();
    let schema_manager = schema_manager(&connection).await;

    let table = Table::editor()
        .set_unquoted_name("my_enum_table", None)
        .set_columns(vec![column("suit", Types::ENUM).create().unwrap()])
        .create()
        .unwrap();

    let e = schema_manager.create_table(&table).await.unwrap_err();
    assert!(e.is_instance_of(Exception::COLUMN_VALUES_REQUIRED), "{e}");
}
