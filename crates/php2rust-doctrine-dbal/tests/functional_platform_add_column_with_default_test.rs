//! Port of tests/Functional/Platform/AddColumnWithDefaultTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's; the
//! `#[TestWith]` cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/AddColumnWithDefaultTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, CurrentTimestamp, Table};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, exec, schema_manager, sqlite};

/// The `add_default_test` table with its `original_field` column.
fn add_default_test() -> Table {
    Table::editor()
        .set_unquoted_name("add_default_test", None)
        .set_columns(vec![
            column("original_field", Types::STRING)
                .set_length(Some(8))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap()
}

#[tokio::test]
async fn testAddColumnWithDefault() {
    let connection = sqlite();
    let schema_manager = schema_manager(&connection).await;

    let table = add_default_test();

    drop_and_create_table(&connection, &table).await;

    exec(
        &connection,
        "INSERT INTO add_default_test (original_field) VALUES ('one')",
    )
    .await;

    let table = table
        .edit()
        .unwrap()
        .add_column(
            column("new_field", Types::STRING)
                .set_length(Some(8))
                .set_default_value(Value::string("DEFAULT"))
                .create()
                .unwrap(),
        )
        .create()
        .unwrap();

    let diff = schema_manager
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(
            &schema_manager
                .introspect_table_by_unquoted_name("add_default_test", None)
                .await
                .unwrap(),
            &table,
        )
        .unwrap();

    schema_manager.alter_table(&diff).await.unwrap();

    let query = "SELECT original_field, new_field FROM add_default_test";
    let result = connection
        .fetch_numeric(query, &PhpArray::new(), &ParameterTypes::new())
        .await
        .unwrap();
    let expected = PhpArray::list([Value::string("one"), Value::string("DEFAULT")]);
    assert!(
        result.as_ref().is_some_and(|row| Value::Array(expected.clone())
            .identical(&Value::Array(row.clone()))),
        "{result:?}"
    );
}

#[tokio::test]
async fn testAddColumnWithCurrentTimestampDefaultToPopulatedTable() {
    for type_name in [Types::DATETIME_MUTABLE, Types::DATETIME_IMMUTABLE] {
        let connection = sqlite();
        let schema_manager = schema_manager(&connection).await;

        let table = add_default_test();

        drop_and_create_table(&connection, &table).await;

        let mut data = PhpArray::new();
        data.insert("original_field", Value::string("one"));
        connection
            .insert("add_default_test", &data, &ParameterTypes::new())
            .await
            .unwrap();

        let table = table
            .edit()
            .unwrap()
            .add_column(
                column("created_at", type_name)
                    .set_default_value(CurrentTimestamp::value())
                    .create()
                    .unwrap(),
            )
            .create()
            .unwrap();

        let diff = schema_manager
            .create_comparator()
            .await
            .unwrap()
            .compare_tables(
                &schema_manager
                    .introspect_table_by_unquoted_name("add_default_test", None)
                    .await
                    .unwrap(),
                &table,
            )
            .unwrap();

        // A non-constant default such as CURRENT_TIMESTAMP cannot be applied with
        // ALTER TABLE ... ADD COLUMN on a non-empty table on SQLite; the platform must
        // therefore fall back to rebuilding the table instead of generating invalid SQL.
        schema_manager.alter_table(&diff).await.unwrap();

        let created_at = connection
            .fetch_one(
                "SELECT created_at FROM add_default_test",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        assert!(!matches!(created_at, Value::Null), "{type_name}");
        assert!(!Value::string("").identical(&created_at), "{type_name}");
    }
}
