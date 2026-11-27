//! Port of tests/Functional/Schema/ColumnCommentTest.php on `pdo_sqlite`
//! in memory. Each test has its own database, so the table that upstream's
//! `setUp()` creates once per class is created by the test that reads it.
//! A data provider's cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/ColumnCommentTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

/// `commentProvider()`.
const COMMENTS: [(&str, &str, &str); 4] = [
    ("Empty comment", "empty_comment", ""),
    ("Non-empty comment", "some_comment", ""),
    ("Zero comment", "zero_comment", "0"),
    ("Comment with quote", "quoted_comment", "O'Reilly"),
];

/// `alterColumnCommentProvider()`.
const ALTERED_COMMENTS: [(&str, &str, &str); 5] = [
    ("Empty to non-empty", "", "foo"),
    ("Non-empty to empty", "foo", ""),
    ("Empty to zero", "", "0"),
    ("Zero to empty", "0", ""),
    ("Non-empty to non-empty", "foo", "bar"),
];

/// `assertColumnComment($columnName, $expectedComment)`.
async fn assert_column_comment(
    connection: &Connection,
    column_name: &str,
    expected: &str,
    case: &str,
) {
    assert_eq!(
        expected,
        schema_manager(connection)
            .await
            .introspect_table_by_unquoted_name("column_comments", None)
            .await
            .unwrap()
            .get_column(column_name)
            .unwrap()
            .get_comment(),
        "{case}"
    );
}

#[tokio::test]
async fn testColumnComment() {
    let connection = sqlite();

    // setUp()
    let mut editor = Table::editor()
        .set_unquoted_name("column_comments", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()]);

    for (_, column_name, comment) in COMMENTS {
        editor = editor.add_column(
            column(column_name, Types::INTEGER)
                .set_comment(comment)
                .create()
                .unwrap(),
        );
    }

    let table = editor.create().unwrap();
    drop_and_create_table(&connection, &table).await;

    for (case, column_name, comment) in COMMENTS {
        assert_column_comment(&connection, column_name, comment, case).await;
    }
}

#[tokio::test]
async fn testAlterColumnComment() {
    for (case, comment1, comment2) in ALTERED_COMMENTS {
        let connection = sqlite();
        let table1 = Table::editor()
            .set_unquoted_name("column_comments", None)
            .set_columns(vec![
                column("id", Types::INTEGER)
                    .set_comment(comment1)
                    .create()
                    .unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table1).await;

        let table2 = table1
            .edit()
            .unwrap()
            .modify_column_by_unquoted_name("id", |editor| editor.set_comment(comment2))
            .create()
            .unwrap();

        let schema_manager = schema_manager(&connection).await;
        let diff = schema_manager
            .create_comparator()
            .await
            .unwrap()
            .compare_tables(&table1, &table2)
            .unwrap();

        schema_manager.alter_table(&diff).await.unwrap();

        assert_column_comment(&connection, "id", comment2, case).await;
    }
}

#[tokio::test]
async fn testAddColumnWithComment() {
    let connection = sqlite();
    let table1 = Table::editor()
        .set_unquoted_name("column_comments", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table1).await;

    let table2 = table1
        .edit()
        .unwrap()
        .add_column(
            column("added", Types::INTEGER)
                .set_comment("added column comment")
                .create()
                .unwrap(),
        )
        .create()
        .unwrap();

    let schema_manager = schema_manager(&connection).await;
    let diff = schema_manager
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&table1, &table2)
        .unwrap();

    schema_manager.alter_table(&diff).await.unwrap();

    assert_column_comment(&connection, "added", "added column comment", "").await;
}
