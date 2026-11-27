//! Port of tests/Functional/Platform/AlterColumnLengthChangeTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/AlterColumnLengthChangeTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

#[tokio::test]
async fn testColumnLengthIsChanged() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("test_alter_length", None)
        .set_columns(vec![
            column("c1", Types::STRING)
                .set_length(Some(50))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let sm = schema_manager(&connection).await;
    let table = sm
        .introspect_table_by_unquoted_name("test_alter_length", None)
        .await
        .unwrap();
    let columns = table.get_columns();
    assert_eq!(1, columns.len());
    assert_eq!(Some(50), columns[0].get_length());

    let table = table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("c1", |editor| editor.set_length(Some(100)))
        .create()
        .unwrap();

    let diff = sm
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(
            &sm.introspect_table_by_unquoted_name("test_alter_length", None)
                .await
                .unwrap(),
            &table,
        )
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name("test_alter_length", None)
        .await
        .unwrap();
    let columns = table.get_columns();

    assert_eq!(1, columns.len());
    assert_eq!(Some(100), columns[0].get_length());
}
