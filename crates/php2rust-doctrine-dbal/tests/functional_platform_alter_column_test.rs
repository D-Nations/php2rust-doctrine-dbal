//! Port of tests/Functional/Platform/AlterColumnTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's. `assertUnqualifiedNameListEquals()`
//! compares the identifiers' values: `SQLite` does not fold unquoted
//! names, so both lists quote to the same names. The two collation tests
//! run on `PostgreSQL` only (parity marks them n/a).
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/AlterColumnTest.php
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
async fn testColumnPositionRetainedAfterAltering() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("test_alter", None)
        .set_columns(vec![
            column("c1", Types::INTEGER).create().unwrap(),
            column("c2", Types::INTEGER).create().unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("c1", |editor| {
            editor.set_type_name(Types::STRING).set_length(Some(16))
        })
        .create()
        .unwrap();

    let sm = schema_manager(&connection).await;
    let diff = sm
        .create_comparator()
        .compare_tables(
            &sm.introspect_table_by_unquoted_name("test_alter", None)
                .await
                .unwrap(),
            &table,
        )
        .unwrap();

    sm.alter_table(&diff).await.unwrap();

    let table = sm
        .introspect_table_by_unquoted_name("test_alter", None)
        .await
        .unwrap();

    let names: Vec<String> = table
        .get_columns()
        .iter()
        .map(|column| {
            column
                .get_object_name()
                .unwrap()
                .get_identifier()
                .get_value()
                .to_owned()
        })
        .collect();
    assert_eq!(vec!["c1", "c2"], names);
}
