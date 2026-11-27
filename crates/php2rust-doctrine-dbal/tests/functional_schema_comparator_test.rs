//! Port of tests/Functional/Schema/ComparatorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's; a data provider's (or
//! `#[TestWith]`'s) cases run in one test, in order. PHP's
//! `getChangedColumns()` keys are the lowercased old column names; the
//! port's list is searched by them. `assertSame()` of the renamed
//! columns compares their keys and `Debug` output.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/ComparatorTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod comparator_test_utils;
mod schema_manager_support;

use comparator_test_utils::{
    assert_diff_not_empty, diff_from_actual_to_desired_table, diff_from_desired_to_actual_table,
};
use php_value::Value;
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Column, ColumnDiff, Comparator, ComparatorConfig, Table, TableDiff,
};
use php2rust_doctrine_dbal::types::{self, Types};
use schema_manager_support::{
    assert_unqualified_name_equals, drop_and_create_table, platform, schema_manager, sqlite,
};

/// `defaultValueProvider()`.
fn default_value_provider() -> Vec<(&'static str, Value)> {
    vec![
        (Types::INTEGER, Value::Int(1)),
        (Types::FLOAT, Value::Float(14.75)),
        (Types::FLOAT, Value::string("50.0")),
        (Types::SMALLFLOAT, Value::Float(14.75)),
        (Types::BOOLEAN, Value::Bool(false)),
        (Types::TEXT, Value::string("Doctrine")),
    ]
}

#[tokio::test]
async fn testDefaultValueComparison() {
    for (type_name, value) in default_value_provider() {
        let connection = sqlite();
        let schema_manager = schema_manager(&connection).await;

        let table = Table::editor()
            .set_unquoted_name("default_value", None)
            .set_columns(vec![
                Column::editor()
                    .set_unquoted_name("test")
                    .set_type_name(type_name)
                    .set_default_value(value.clone())
                    .create()
                    .unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let online_table = schema_manager
            .introspect_table_by_unquoted_name("default_value", None)
            .await
            .unwrap();

        assert!(
            schema_manager
                .create_comparator()
                .await
                .unwrap()
                .compare_tables(&online_table, &table)
                .unwrap()
                .is_empty(),
            "{type_name} {value:?}"
        );
    }
}

#[tokio::test]
async fn testFloatDefaultValueComparisonConverges() {
    for type_name in [Types::FLOAT, Types::SMALLFLOAT] {
        let connection = sqlite();
        let schema_manager = schema_manager(&connection).await;

        let table = Table::editor()
            .set_unquoted_name("float_default_value", None)
            .set_columns(vec![
                Column::editor()
                    .set_unquoted_name("score")
                    .set_type_name(type_name)
                    .set_default_value(Value::Float(14.75))
                    .create()
                    .unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let comparator = schema_manager.create_comparator().await.unwrap();

        assert!(
            diff_from_actual_to_desired_table(&schema_manager, &comparator, &table)
                .await
                .unwrap()
                .is_empty()
        );

        assert!(
            diff_from_desired_to_actual_table(&schema_manager, &comparator, &table)
                .await
                .unwrap()
                .is_empty()
        );

        let desired_table = table
            .edit()
            .unwrap()
            .modify_column_by_unquoted_name("score", |editor| {
                editor.set_default_value(Value::Float(50.0))
            })
            .create()
            .unwrap();

        assert_diff_not_empty(&connection, &comparator, &desired_table).await;
    }
}

/// `$compareResult->getChangedColumns()[$key]`.
fn changed_column<'d>(diff: &'d TableDiff, key: &str) -> &'d ColumnDiff {
    diff.get_changed_columns()
        .iter()
        .find(|d| d.get_old_column().get_name().to_ascii_lowercase() == key)
        .expect("array has key")
}

/// `RenameColumnTest::getRenamedColumns($tableDiff)`.
fn get_renamed_columns(table_diff: &TableDiff) -> Vec<(String, &Column)> {
    let mut renamed: Vec<(String, &Column)> = Vec::new();
    for diff in table_diff.get_changed_columns() {
        if !diff.has_name_changed() {
            continue;
        }
        let old_column_name = diff.get_old_column().get_object_name().unwrap().to_string();
        match renamed.iter_mut().find(|(k, _)| *k == old_column_name) {
            Some(entry) => entry.1 = diff.get_new_column(),
            None => renamed.push((old_column_name, diff.get_new_column())),
        }
    }
    renamed
}

#[tokio::test]
async fn testRenameColumnComparison() {
    let connection = sqlite();
    let platform = platform(&connection).await;
    let comparator = Comparator::new(platform.as_dyn(), ComparatorConfig::default());

    let string = |name: &str, length: i64, default: &str| {
        Column::editor()
            .set_unquoted_name(name)
            .set_type_name(Types::STRING)
            .set_length(Some(length))
            .set_default_value(Value::string(default))
            .create()
            .unwrap()
    };

    let mut table = Table::editor()
        .set_unquoted_name("rename_table", None)
        .set_columns(vec![
            string("test", 20, "baz"),
            string("test2", 20, "baz"),
            string("test3", 10, "foo"),
        ])
        .create()
        .unwrap();

    let online_table = table.clone();
    table
        .rename_column("test", "baz")
        .unwrap()
        .set_length(Some(40))
        .set_comment("Comment");

    table.rename_column("test2", "foo").unwrap();

    table
        .get_column_mut("test3")
        .unwrap()
        .set_autoincrement(true)
        .set_notnull(false)
        .set_type(&types::get_type(Types::BIGINT).unwrap())
        .unwrap();

    let compare_result = comparator.compare_tables(&online_table, &table).unwrap();
    let renamed_columns = get_renamed_columns(&compare_result);
    let show = |columns: &[(String, &Column)]| -> Vec<(String, String)> {
        columns
            .iter()
            .map(|(k, c)| (k.clone(), format!("{c:?}")))
            .collect()
    };
    assert_eq!(
        show(&renamed_columns),
        show(&compare_result.get_renamed_columns())
    );
    assert_eq!(compare_result.get_changed_columns().len(), 3);
    assert_eq!(compare_result.get_modified_columns().len(), 2);
    assert_eq!(renamed_columns.len(), 2);
    assert!(renamed_columns.iter().any(|(k, _)| k == "test2"));

    let renamed_only = changed_column(&compare_result, "test2");
    let renamed_and_modified = changed_column(&compare_result, "test");
    let modified_only = changed_column(&compare_result, "test3");

    assert_unqualified_name_equals(
        "test2",
        &renamed_only.get_new_column().get_object_name().unwrap(),
    );

    assert!(renamed_only.has_name_changed());
    assert_eq!(1, renamed_only.count_changed_properties());

    assert_unqualified_name_equals(
        "test",
        &renamed_and_modified
            .get_new_column()
            .get_object_name()
            .unwrap(),
    );

    assert!(renamed_and_modified.has_name_changed());
    assert!(renamed_and_modified.has_length_changed());
    assert!(renamed_and_modified.has_comment_changed());
    assert!(!renamed_and_modified.has_type_changed());
    assert_eq!(3, renamed_and_modified.count_changed_properties());

    assert!(modified_only.has_auto_increment_changed());
    assert!(modified_only.has_not_null_changed());
    assert!(modified_only.has_type_changed());
    assert!(!modified_only.has_length_changed());
    assert!(!modified_only.has_comment_changed());
    assert!(!modified_only.has_name_changed());
    assert_eq!(3, modified_only.count_changed_properties());
}
