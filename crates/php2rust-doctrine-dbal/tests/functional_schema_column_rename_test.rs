//! Port of tests/Functional/Schema/ColumnRenameTest.php on `pdo_sqlite` in
//! memory.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/ColumnRenameTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Index, Table, TableEditor, UniqueConstraint,
};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_table_if_exists, foreign_key, schema_manager, sqlite};

/// `testRenameColumn($modifier)`.
async fn test_rename_column(
    connection: &Connection,
    modifier: impl FnOnce(TableEditor) -> TableEditor,
) {
    let schema_manager = schema_manager(connection).await;
    let comparator = schema_manager.create_comparator().await.unwrap();

    drop_table_if_exists(connection, "rename_column").await;

    let editor = Table::editor()
        .set_unquoted_name("rename_column", None)
        .set_columns(vec![
            column("c1", Types::INTEGER).create().unwrap(),
            column("c2", Types::INTEGER).create().unwrap(),
        ]);

    let mut table = modifier(editor).create().unwrap();
    table.rename_column("c1", "c1a").unwrap();

    schema_manager.create_table(&table).await.unwrap();

    assert!(
        comparator
            .compare_tables(
                &table,
                &schema_manager
                    .introspect_table_by_unquoted_name("rename_column", None)
                    .await
                    .unwrap(),
            )
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn testRenameColumnInIndex() {
    let connection = sqlite();
    test_rename_column(&connection, |editor| {
        editor.add_index(
            Index::editor()
                .set_unquoted_name("idx_c1_c2")
                .set_unquoted_column_names(&["c1", "c1"])
                .create()
                .unwrap(),
        )
    })
    .await;
}

#[tokio::test]
async fn testRenameColumnInForeignKeyConstraint() {
    let connection = sqlite();
    drop_table_if_exists(&connection, "rename_column_referenced").await;

    let referenced_table = Table::editor()
        .set_unquoted_name("rename_column_referenced", None)
        .set_columns(vec![
            column("c1", Types::INTEGER).create().unwrap(),
            column("c2", Types::INTEGER).create().unwrap(),
        ])
        // PostgreSQL requires a unique constraint on the referenced table
        // columns
        .set_unique_constraints(vec![
            UniqueConstraint::editor()
                .set_unquoted_column_names(&["c1", "c2"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    schema_manager(&connection)
        .await
        .create_table(&referenced_table)
        .await
        .unwrap();

    test_rename_column(&connection, |editor| {
        editor.add_foreign_key_constraint(foreign_key(
            Some("fk_c1_c2"),
            &["c1", "c2"],
            "rename_column_referenced",
            &["c1", "c2"],
        ))
    })
    .await;
}
