//! Port of tests/Functional/Schema/AlterTableTest.php on `pdo_sqlite` in
//! memory. The tests that upstream skips on `SQLite` are not here (parity
//! marks them n/a).
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/AlterTableTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, ComparatorConfig, Table, TableEditor, UniqueConstraint,
};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{
    column, drop_and_create_table, drop_table_if_exists, foreign_key, primary_key, schema_manager,
    sqlite,
};

/// `testMigration($oldTable, $migration, $config)`.
async fn test_migration(
    connection: &Connection,
    old_table: &Table,
    migration: impl FnOnce(TableEditor) -> TableEditor,
    config: Option<ComparatorConfig>,
) {
    drop_and_create_table(connection, old_table).await;

    let editor = old_table.edit().unwrap();
    let new_table = migration(editor).create().unwrap();

    let schema_manager = schema_manager(connection).await;
    let diff = schema_manager
        .create_comparator_with(config.unwrap_or_default())
        .compare_tables(old_table, &new_table)
        .unwrap();

    assert!(!diff.is_empty());

    schema_manager.alter_table(&diff).await.unwrap();

    let introspected_table = schema_manager
        .introspect_table(&new_table.get_object_name().unwrap().to_string())
        .await
        .unwrap();

    let diff = schema_manager
        .create_comparator()
        .compare_tables(&new_table, &introspected_table)
        .unwrap();

    assert!(diff.is_empty());
}

#[tokio::test]
async fn testAddPrimaryKeyOnNewAutoIncrementColumn() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("alter_pk", None)
        .set_columns(vec![column("val", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    test_migration(
        &connection,
        &table,
        |editor| {
            editor
                .add_column(
                    column("id", Types::INTEGER)
                        .set_autoincrement(true)
                        .create()
                        .unwrap(),
                )
                .set_primary_key_constraint(primary_key(&["id"]))
        },
        None,
    )
    .await;
}

#[tokio::test]
async fn testAddNewColumnToPrimaryKey() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("alter_pk", None)
        .set_columns(vec![column("id1", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id1"]))
        .create()
        .unwrap();

    test_migration(
        &connection,
        &table,
        |editor| {
            editor
                .add_column(column("id2", Types::INTEGER).create().unwrap())
                .drop_primary_key_constraint()
                .add_primary_key_constraint(primary_key(&["id1", "id2"]).unwrap())
        },
        None,
    )
    .await;
}

#[tokio::test]
async fn testReplaceForeignKeyConstraint() {
    let connection = sqlite();
    let articles = Table::editor()
        .set_unquoted_name("articles", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("sku", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_unique_constraints(vec![
            UniqueConstraint::editor()
                .set_unquoted_column_names(&["sku"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let orders = Table::editor()
        .set_unquoted_name("orders", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("article_id", Types::INTEGER).create().unwrap(),
            column("article_sku", Types::INTEGER).create().unwrap(),
        ])
        .set_foreign_key_constraints(vec![foreign_key(
            Some("articles_fk"),
            &["article_id"],
            "articles",
            &["id"],
        )])
        .create()
        .unwrap();

    drop_table_if_exists(&connection, "orders").await;
    drop_table_if_exists(&connection, "articles").await;

    schema_manager(&connection)
        .await
        .create_table(&articles)
        .await
        .unwrap();

    test_migration(
        &connection,
        &orders,
        |editor| {
            editor
                .drop_foreign_key_constraint_by_unquoted_name("articles_fk")
                .add_foreign_key_constraint(foreign_key(
                    Some("articles_fk"),
                    &["article_sku"],
                    "articles",
                    &["sku"],
                ))
        },
        None,
    )
    .await;
}
