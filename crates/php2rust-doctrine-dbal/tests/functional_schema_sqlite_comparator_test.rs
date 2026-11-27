//! Port of tests/Functional/Schema/SQLite/ComparatorTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/SQLite/ComparatorTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod comparator_test_utils;
mod schema_manager_support;

use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

#[tokio::test]
async fn testChangeTableCollation() {
    let connection = sqlite();
    let schema_manager = schema_manager(&connection).await;
    let comparator = schema_manager.create_comparator().await.unwrap();

    let table = Table::editor()
        .set_unquoted_name("comparator_test", None)
        .set_columns(vec![column("id", Types::STRING).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = table
        .edit()
        .unwrap()
        .modify_column_by_unquoted_name("id", |editor| editor.set_collation(Some("NOCASE")))
        .create()
        .unwrap();

    comparator_test_utils::assert_diff_not_empty(&connection, &comparator, &table).await;
}
