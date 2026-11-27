//! Port of tests/Functional/Platform/RenameColumnTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's; a data provider's cases run in one
//! test, in order. `assertUnqualifiedNameListEquals()` compares the
//! identifiers' values: `SQLite` does not fold unquoted names, so both
//! lists quote to the same names. `assertEquals()` is PHPUnit's loose
//! comparison.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/RenameColumnTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Column, Table, TableDiff};
use php2rust_doctrine_dbal::types::{self, Types};
use schema_manager_support::{column, drop_and_create_table, platform, schema_manager, sqlite};

/// `columnNameProvider()`.
const COLUMN_NAME_PROVIDER: [(&str, &str); 3] = [
    ("c1", "c1_x"),
    ("C1", "c1_x"),
    ("importantColumn", "very_important_column"),
];

/// The names of a table's columns, in order.
fn column_names(table: &Table) -> Vec<String> {
    table
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
        .collect()
}

/// `getRenamedColumns($tableDiff)`: old column name => new column.
fn get_renamed_columns(table_diff: &TableDiff) -> Vec<(String, &Column)> {
    let mut renamed: Vec<(String, &Column)> = Vec::new();
    for diff in table_diff.get_changed_columns() {
        if !diff.has_name_changed() {
            continue;
        }

        let old_column_name = diff.get_old_column().get_object_name().unwrap().to_string();

        match renamed
            .iter_mut()
            .find(|(name, _)| *name == old_column_name)
        {
            Some(entry) => entry.1 = diff.get_new_column(),
            None => renamed.push((old_column_name, diff.get_new_column())),
        }
    }

    renamed
}

#[tokio::test]
async fn testColumnPositionRetainedAfterImplicitRenaming() {
    for (old_column_name, new_column_name) in COLUMN_NAME_PROVIDER {
        let connection = sqlite();
        let table = Table::editor()
            .set_unquoted_name("test_rename", None)
            .set_columns(vec![
                column(old_column_name, Types::STRING)
                    .set_length(Some(16))
                    .create()
                    .unwrap(),
                column("c2", Types::INTEGER).create().unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let table = table
            .edit()
            .unwrap()
            .drop_column_by_unquoted_name(old_column_name)
            .add_column(
                column(new_column_name, Types::STRING)
                    .set_length(Some(16))
                    .create()
                    .unwrap(),
            )
            .create()
            .unwrap();

        let sm = schema_manager(&connection).await;
        let diff = sm
            .create_comparator()
            .compare_tables(
                &sm.introspect_table_by_unquoted_name("test_rename", None)
                    .await
                    .unwrap(),
                &table,
            )
            .unwrap();

        sm.alter_table(&diff).await.unwrap();

        let table = sm
            .introspect_table_by_unquoted_name("test_rename", None)
            .await
            .unwrap();

        assert_eq!(vec![new_column_name, "c2"], column_names(&table));

        assert_eq!(1, get_renamed_columns(&diff).len());
        assert_eq!(1, diff.get_renamed_columns().len());
    }
}

#[tokio::test]
async fn testColumnPositionRetainedAfterExplicitRenaming() {
    for (old_column_name, new_column_name) in COLUMN_NAME_PROVIDER {
        let connection = sqlite();
        let mut table = Table::editor()
            .set_unquoted_name("test_rename", None)
            .set_columns(vec![
                column(old_column_name, Types::INTEGER)
                    .set_length(Some(16))
                    .create()
                    .unwrap(),
                column("c2", Types::INTEGER).create().unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        // Force a different type to make sure it's not being caught implicitly
        table
            .rename_column(old_column_name, new_column_name)
            .unwrap()
            .set_type(&types::get_type(Types::BIGINT).unwrap())
            .unwrap()
            .set_length(Some(32));

        let sm = schema_manager(&connection).await;
        let diff = sm
            .create_comparator()
            .compare_tables(
                &sm.introspect_table_by_unquoted_name("test_rename", None)
                    .await
                    .unwrap(),
                &table,
            )
            .unwrap();

        sm.alter_table(&diff).await.unwrap();

        let table = sm
            .introspect_table_by_unquoted_name("test_rename", None)
            .await
            .unwrap();

        assert_eq!(1, diff.get_changed_columns().len());
        assert_eq!(1, diff.get_renamed_columns().len());
        assert_eq!(1, diff.get_modified_columns().len());

        assert_eq!(vec![new_column_name, "c2"], column_names(&table));
    }
}

#[tokio::test]
async fn testRenameColumToQuoted() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("test_rename", None)
        .set_columns(vec![column("c1", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let table = table
        .edit()
        .unwrap()
        .drop_column_by_unquoted_name("c1")
        .add_column(
            Column::editor()
                .set_quoted_name("c2")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        )
        .create()
        .unwrap();

    let schema_manager = schema_manager(&connection).await;
    let comparator = schema_manager.create_comparator();

    let diff = comparator
        .compare_tables(
            &schema_manager
                .introspect_table_by_unquoted_name("test_rename", None)
                .await
                .unwrap(),
            &table,
        )
        .unwrap();
    assert!(!diff.is_empty());

    schema_manager.alter_table(&diff).await.unwrap();

    let platform = platform(&connection).await;

    let mut data = PhpArray::new();
    data.insert(platform.quote_single_identifier("c2"), Value::Int(1));
    let inserted = connection
        .insert("test_rename", &data, &ParameterTypes::new())
        .await
        .unwrap();
    assert!(phpunit_equals(&Value::Int(1), &Value::Int(inserted)));
}
