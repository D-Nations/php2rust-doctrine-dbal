//! Port of tests/Platforms/SQLite/ComparatorTest.php with the tests it
//! inherits from tests/Schema/AbstractComparatorTestCase.php, at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's. `assertEquals()` of two diffs compares
//! their `Debug` output. The tests that need `Schema`, `Sequence` and
//! `SchemaDiff` (`compareSchemas()`, `diffSequence()`) are not ported
//! yet. `get_renamed_columns()` is the helper of
//! tests/Functional/Platform/RenameColumnTest.php.
//!
//! Upstream: doctrine/dbal tests/Platforms/SQLite/ComparatorTest.php
//! Upstream: doctrine/dbal tests/Schema/AbstractComparatorTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php_value::PhpArray;
use php2rust_doctrine_dbal::platforms::SQLitePlatform;
use php2rust_doctrine_dbal::platforms::sqlite::Comparator;
use php2rust_doctrine_dbal::schema::name::UnqualifiedName;
use php2rust_doctrine_dbal::schema::{
    Column, ColumnDiff, ComparatorConfig, ForeignKeyConstraint, Index, IndexRename, IndexType,
    PrimaryKeyConstraint, ReferentialAction, Table, TableDiff, TableDiffParts,
};
use php2rust_doctrine_dbal::types::Types;
use schema_support::{expect_deprecation, expect_no_deprecation};

static PLATFORM: std::sync::LazyLock<SQLitePlatform> =
    std::sync::LazyLock::new(SQLitePlatform::new);

/// `createComparator($config)`.
fn create_comparator(config: ComparatorConfig) -> Comparator<'static> {
    Comparator::new(&PLATFORM, config)
}

/// `setUp()`: `$this->comparator`.
fn comparator() -> Comparator<'static> {
    create_comparator(ComparatorConfig::default())
}

fn column(name: &str, type_name: &str) -> Column {
    Column::editor()
        .set_unquoted_name(name)
        .set_type_name(type_name)
        .create()
        .unwrap()
}

fn table(name: &str, columns: Vec<Column>) -> Table {
    Table::editor()
        .set_unquoted_name(name, None)
        .set_columns(columns)
        .create()
        .unwrap()
}

fn index(name: &str, column: &str) -> Index {
    Index::editor()
        .set_unquoted_name(name)
        .set_unquoted_column_names(&[column])
        .create()
        .unwrap()
}

fn foreign_key(name: Option<&str>, foreign_table: &str) -> ForeignKeyConstraint {
    let mut editor = ForeignKeyConstraint::editor();
    if let Some(name) = name {
        editor = editor.set_unquoted_name(name);
    }
    editor
        .set_unquoted_referencing_column_names(&["fk"])
        .set_unquoted_referenced_table_name(foreign_table, None)
        .set_unquoted_referenced_column_names(&["id"])
        .create()
        .unwrap()
}

/// `RenameColumnTest::getRenamedColumns($tableDiff)`.
fn get_renamed_columns(table_diff: &TableDiff) -> Vec<(String, Column)> {
    let mut renamed: Vec<(String, Column)> = Vec::new();
    for diff in table_diff.get_changed_columns() {
        if !diff.has_name_changed() {
            continue;
        }
        let old_column_name = diff.get_old_column().get_object_name().unwrap().to_string();
        match renamed.iter_mut().find(|(k, _)| *k == old_column_name) {
            Some(entry) => entry.1 = diff.get_new_column().clone(),
            None => renamed.push((old_column_name, diff.get_new_column().clone())),
        }
    }
    renamed
}

/// `getObjectNames($objects)`.
fn get_object_names(columns: &[Column]) -> Vec<String> {
    columns
        .iter()
        .map(|c| c.get_object_name().unwrap().to_string())
        .collect()
}

fn get_index_names(indexes: &[Index]) -> Vec<String> {
    indexes
        .iter()
        .map(|i| i.get_object_name().unwrap().to_string())
        .collect()
}

fn assert_diff_equals(expected: &TableDiff, actual: &TableDiff) {
    assert_eq!(format!("{expected:?}"), format!("{actual:?}"));
}

#[test]
fn testCompareAutoIncrementChanged() {
    let column1 = Column::editor()
        .set_unquoted_name("foo")
        .set_type_name(Types::INTEGER)
        .set_autoincrement(true)
        .create()
        .unwrap();

    let column2 = column1
        .edit()
        .unwrap()
        .set_autoincrement(false)
        .create()
        .unwrap();

    let diff = ColumnDiff::new(column2, column1);

    assert!(diff.has_auto_increment_changed());
}

#[test]
fn testCompareChangedColumnsChangeType() {
    let column1 = column("id", Types::STRING);

    let column2 = column1
        .edit()
        .unwrap()
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap();

    let diff12 = ColumnDiff::new(column2, column1.clone());
    assert!(diff12.has_type_changed());

    let diff11 = ColumnDiff::new(column1.clone(), column1);
    assert!(!diff11.has_type_changed());
}

#[test]
fn testSameTypeNameIsNotAChange() {
    let column1 = column("id", Types::INTEGER);

    let column2 = column1
        .edit()
        .unwrap()
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap();

    let diff = ColumnDiff::new(column2, column1);
    assert!(!diff.has_type_changed());
}

#[test]
fn testCompareChangeColumnsMultipleNewColumnsRename() {
    let table_a = table("foo", vec![column("datecolumn1", Types::DATETIME_MUTABLE)]);

    let table_b = table(
        "foo",
        vec![
            column("new_datecolumn1", Types::DATETIME_MUTABLE),
            column("new_datecolumn2", Types::DATETIME_MUTABLE),
        ],
    );

    let table_diff = comparator().compare_tables(&table_a, &table_b).unwrap();

    let renamed_columns = get_renamed_columns(&table_diff);
    assert_eq!(renamed_columns.len(), 1);
    assert!(renamed_columns.iter().any(|(k, _)| k == "datecolumn1"));
    assert_eq!(
        vec!["new_datecolumn2"],
        get_object_names(table_diff.get_added_columns())
    );

    assert_eq!(table_diff.get_dropped_columns().len(), 0);
    assert_eq!(table_diff.get_changed_columns().len(), 1);
}

#[test]
fn testTableAddForeignKey() {
    let _table_foreign = table("bar", vec![column("id", Types::INTEGER)]);

    let table1 = table("foo", vec![column("fk", Types::INTEGER)]);

    let table2 = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("fk", Types::INTEGER)])
        .set_foreign_key_constraints(vec![foreign_key(None, "bar")])
        .create()
        .unwrap();

    let table_diff = comparator().compare_tables(&table1, &table2).unwrap();

    assert_eq!(table_diff.get_added_foreign_keys().len(), 1);
}

#[test]
fn testTableRemoveForeignKey() {
    let table1 = table("foo", vec![column("fk", Types::INTEGER)]);

    let table2 = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("fk", Types::INTEGER)])
        .set_foreign_key_constraints(vec![foreign_key(Some("fk_bar"), "bar")])
        .create()
        .unwrap();

    let table_diff = comparator().compare_tables(&table2, &table1).unwrap();

    assert_eq!(
        table_diff
            .get_dropped_foreign_key_constraint_names()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn testTableUpdateForeignKey() {
    let table1 = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("fk", Types::INTEGER)])
        .set_foreign_key_constraints(vec![foreign_key(Some("fk_bar"), "bar")])
        .create()
        .unwrap();

    let table2 = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("fk", Types::INTEGER)])
        .set_foreign_key_constraints(vec![
            ForeignKeyConstraint::editor()
                .set_unquoted_referencing_column_names(&["fk"])
                .set_unquoted_referenced_table_name("bar", None)
                .set_unquoted_referenced_column_names(&["id"])
                .set_on_update_action(ReferentialAction::Cascade)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let table_diff = comparator().compare_tables(&table1, &table2).unwrap();

    assert_eq!(
        table_diff
            .get_dropped_foreign_key_constraint_names()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(table_diff.get_added_foreign_keys().len(), 1);
}

#[test]
fn testMovedForeignKeyForeignTable() {
    let table1 = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("fk", Types::INTEGER)])
        .set_foreign_key_constraints(vec![foreign_key(Some("fk_bar"), "bar")])
        .create()
        .unwrap();

    let table2 = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("fk", Types::INTEGER)])
        .set_foreign_key_constraints(vec![foreign_key(None, "bar2")])
        .create()
        .unwrap();

    let table_diff = comparator().compare_tables(&table1, &table2).unwrap();

    assert_eq!(
        table_diff
            .get_dropped_foreign_key_constraint_names()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(table_diff.get_added_foreign_keys().len(), 1);
}

#[test]
fn testCompareColumnCompareCaseInsensitive() {
    let table_a = table("foo", vec![column("id", Types::INTEGER)]);
    let table_b = table("foo", vec![column("ID", Types::INTEGER)]);

    let table_diff = comparator().compare_tables(&table_a, &table_b).unwrap();

    assert!(table_diff.is_empty());
}

#[test]
fn testCompareIndexBasedOnPropertiesNotName() {
    let table_a = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("id", Types::INTEGER)])
        .set_indexes(vec![index("foo_bar_idx", "id")])
        .create()
        .unwrap();

    let table_b = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("ID", Types::INTEGER)])
        .set_indexes(vec![index("bar_foo_idx", "id")])
        .create()
        .unwrap();

    assert_diff_equals(
        &TableDiff::new(
            table_a.clone(),
            TableDiffParts {
                renamed_indexes: vec![(
                    "foo_bar_idx".to_owned(),
                    Index::new(
                        Some("bar_foo_idx"),
                        ["id"],
                        false,
                        false,
                        Vec::<String>::new(),
                        PhpArray::new(),
                    )
                    .unwrap(),
                )],
                ..TableDiffParts::default()
            },
        ),
        &comparator().compare_tables(&table_a, &table_b).unwrap(),
    );
}

#[test]
fn testCompareForeignKeyBasedOnPropertiesNotName() {
    let fk = |name: &str| {
        ForeignKeyConstraint::editor()
            .set_unquoted_name(name)
            .set_unquoted_referencing_column_names(&["id"])
            .set_unquoted_referenced_table_name("bar", None)
            .set_unquoted_referenced_column_names(&["id"])
            .create()
            .unwrap()
    };

    let table_a = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("id", Types::INTEGER)])
        .set_foreign_key_constraints(vec![fk("foo_constraint")])
        .create()
        .unwrap();

    let table_b = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("ID", Types::INTEGER)])
        .set_foreign_key_constraints(vec![fk("bar_constraint")])
        .create()
        .unwrap();

    assert_diff_equals(
        &TableDiff::new(table_a.clone(), TableDiffParts::default()),
        &comparator().compare_tables(&table_a, &table_b).unwrap(),
    );
}

#[test]
fn testDetectRenameColumn() {
    let table_a = table("foo", vec![column("foo", Types::INTEGER)]);
    let table_b = table("foo", vec![column("bar", Types::INTEGER)]);

    let table_diff = comparator().compare_tables(&table_a, &table_b).unwrap();

    assert_eq!(table_diff.get_added_columns().len(), 0);
    assert_eq!(table_diff.get_dropped_columns().len(), 0);

    let renamed_columns = get_renamed_columns(&table_diff);
    let renamed = renamed_columns
        .iter()
        .find(|(k, _)| k == "foo")
        .map(|(_, c)| c)
        .expect("array has key foo");
    assert_eq!("bar", renamed.get_object_name().unwrap().to_string());
}

#[test]
fn testDetectRenameColumnDisabled() {
    let table_a = table("foo", vec![column("foo", Types::INTEGER)]);
    let table_b = table("foo", vec![column("bar", Types::INTEGER)]);

    let comparator =
        create_comparator(ComparatorConfig::default().with_detect_renamed_columns(false));
    let table_diff = comparator.compare_tables(&table_a, &table_b).unwrap();

    assert_eq!(table_diff.get_added_columns().len(), 1);
    assert_eq!(table_diff.get_dropped_columns().len(), 1);
    assert_eq!(table_diff.get_renamed_columns().len(), 0);
}

/// You can easily have ambiguities in the column renaming. If these are
/// detected no renaming should take place, instead adding and dropping
/// should be used exclusively.
#[test]
fn testDetectRenameColumnAmbiguous() {
    let table_a = table(
        "foo",
        vec![column("foo", Types::INTEGER), column("bar", Types::INTEGER)],
    );
    let table_b = table("foo", vec![column("baz", Types::INTEGER)]);

    let table_diff = comparator().compare_tables(&table_a, &table_b).unwrap();

    assert_eq!(
        vec!["baz"],
        get_object_names(table_diff.get_added_columns())
    );
    assert_eq!(
        vec!["foo", "bar"],
        get_object_names(table_diff.get_dropped_columns())
    );
    assert_eq!(get_renamed_columns(&table_diff).len(), 0);
}

fn prototype() -> Table {
    table("foo", vec![column("foo", Types::INTEGER)])
}

fn with_indexes(prototype: &Table, names: &[&str]) -> Table {
    let mut editor = prototype.edit().unwrap();
    for name in names {
        editor = editor.add_index(index(name, "foo"));
    }
    editor.create().unwrap()
}

#[test]
fn testDetectRenameIndex() {
    let prototype = prototype();
    let table1 = with_indexes(&prototype, &["idx_foo"]);
    let table2 = with_indexes(&prototype, &["idx_bar"]);

    let table_diff = comparator().compare_tables(&table1, &table2).unwrap();

    assert_eq!(table_diff.get_added_columns().len(), 0);
    assert_eq!(table_diff.get_dropped_indexes().len(), 0);

    let renamed_indexes = table_diff.get_renamed_indexes();
    let renamed = renamed_indexes
        .iter()
        .find(|(k, _)| k == "idx_foo")
        .map(|(_, i)| i)
        .expect("array has key idx_foo");
    assert_eq!("idx_bar", renamed.get_object_name().unwrap().to_string());

    assert_eq!(
        vec![IndexRename::new(
            UnqualifiedName::unquoted("idx_foo").unwrap(),
            index("idx_bar", "foo"),
        )],
        table_diff.get_index_renames().unwrap()
    );
}

#[test]
fn testDetectRenameIndexDisabled() {
    let prototype = prototype();
    let table1 = with_indexes(&prototype, &["idx_foo"]);
    let table2 = with_indexes(&prototype, &["idx_bar"]);

    let comparator =
        create_comparator(ComparatorConfig::default().with_detect_renamed_indexes(false));
    let table_diff = comparator.compare_tables(&table1, &table2).unwrap();

    assert_eq!(table_diff.get_added_indexes().len(), 1);
    assert_eq!(table_diff.get_dropped_indexes().len(), 1);
    assert_eq!(table_diff.get_renamed_indexes().len(), 0);
}

/// You can easily have ambiguities in the index renaming. If these are
/// detected no renaming should take place, instead adding and dropping
/// should be used exclusively.
#[test]
fn testDetectRenameIndexAmbiguous() {
    let prototype = prototype();
    let table1 = with_indexes(&prototype, &["idx_foo", "idx_bar"]);
    let table2 = with_indexes(&prototype, &["idx_baz"]);

    let table_diff = comparator().compare_tables(&table1, &table2).unwrap();

    assert_eq!(
        vec!["idx_baz"],
        get_index_names(table_diff.get_added_indexes())
    );
    assert_eq!(
        vec!["idx_foo", "idx_bar"],
        get_index_names(table_diff.get_dropped_indexes())
    );
    assert_eq!(table_diff.get_renamed_indexes().len(), 0);
}

#[test]
fn testDetectRenameIndexAmbiguousWithMultipleAddedIndexes() {
    let prototype = prototype();
    let table1 = with_indexes(&prototype, &["idx_foo"]);
    let table2 = with_indexes(&prototype, &["idx_bar", "idx_baz"]);

    let table_diff = comparator().compare_tables(&table1, &table2).unwrap();

    assert_eq!(
        vec!["idx_bar", "idx_baz"],
        get_index_names(table_diff.get_added_indexes())
    );
    assert_eq!(
        vec!["idx_foo"],
        get_index_names(table_diff.get_dropped_indexes())
    );
    assert_eq!(table_diff.get_renamed_indexes().len(), 0);
}

#[test]
fn testDetectChangeIdentifierType() {
    let table_a = table("foo", vec![column("id", Types::INTEGER)]);
    let table_b = table(
        "foo",
        vec![
            Column::editor()
                .set_unquoted_name("id")
                .set_type_name(Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ],
    );

    let table_diff = comparator().compare_tables(&table_a, &table_b).unwrap();

    let modified_columns = table_diff.get_changed_columns();
    assert_eq!(modified_columns.len(), 1);
    let modified_column = &modified_columns[0];
    assert_eq!(
        "id",
        modified_column
            .get_old_column()
            .get_object_name()
            .unwrap()
            .to_string()
    );
}

#[test]
fn testReportModifiedIndexesEnabled() {
    let table_diff = expect_deprecation("https://github.com/doctrine/dbal/pull/6890", || {
        compare_tables_with_modified_index(true)
    });

    assert_eq!(table_diff.get_dropped_indexes().len(), 0);
    assert_eq!(table_diff.get_added_indexes().len(), 0);
    assert_eq!(table_diff.get_modified_indexes().len(), 1);
}

#[test]
fn testReportModifiedIndexesDisabled() {
    let table_diff = expect_no_deprecation("https://github.com/doctrine/dbal/pull/6890", || {
        compare_tables_with_modified_index(false)
    });

    assert_eq!(table_diff.get_dropped_indexes().len(), 1);
    assert_eq!(table_diff.get_added_indexes().len(), 1);
    assert_eq!(table_diff.get_modified_indexes().len(), 0);
}

fn compare_tables_with_modified_index(report_modified_indexes: bool) -> TableDiff {
    let table_a = Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![column("id", Types::INTEGER)])
        .set_indexes(vec![index("idx_id", "id")])
        .create()
        .unwrap();

    let table_b = table_a
        .edit()
        .unwrap()
        .drop_index_by_unquoted_name("idx_id")
        .add_index(
            Index::editor()
                .set_unquoted_name("idx_id")
                .set_type(IndexType::Unique)
                .set_unquoted_column_names(&["id"])
                .create()
                .unwrap(),
        )
        .create()
        .unwrap();

    create_comparator(
        ComparatorConfig::default().with_report_modified_indexes(report_modified_indexes),
    )
    .compare_tables(&table_a, &table_b)
    .unwrap()
}

#[test]
fn testDiff() {
    let id = || {
        Column::editor()
            .set_unquoted_name("id")
            .set_type_name(Types::INTEGER)
            .set_autoincrement(true)
            .create()
            .unwrap()
    };
    let string32 = |name: &str| {
        Column::editor()
            .set_unquoted_name(name)
            .set_type_name(Types::STRING)
            .set_length(Some(32))
            .create()
            .unwrap()
    };
    let pk = || {
        Some(
            PrimaryKeyConstraint::editor()
                .set_unquoted_column_names(&["id"])
                .create()
                .unwrap(),
        )
    };

    let table = Table::editor()
        .set_unquoted_name("twitter_users", None)
        .set_columns(vec![
            id(),
            column("twitterId", Types::INTEGER),
            string32("displayName"),
        ])
        .set_primary_key_constraint(pk())
        .create()
        .unwrap();

    let newtable = Table::editor()
        .set_unquoted_name("twitter_users", None)
        .set_columns(vec![
            id(),
            column("twitter_id", Types::INTEGER),
            string32("display_name"),
            column("logged_in_at", Types::DATETIME_MUTABLE),
        ])
        .set_primary_key_constraint(pk())
        .create()
        .unwrap();

    let table_diff = comparator().compare_tables(&table, &newtable).unwrap();

    assert_eq!(
        vec!["twitterId", "displayName"],
        get_renamed_columns(&table_diff)
            .into_iter()
            .map(|(k, _)| k)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        vec!["logged_in_at"],
        get_object_names(table_diff.get_added_columns())
    );
    assert_eq!(table_diff.get_dropped_columns().len(), 0);
}

/// `getCompareColumnComments()`.
const COMPARE_COLUMN_COMMENTS: [(&str, &str, bool); 10] = [
    ("", "", true),
    (" ", " ", true),
    ("0", "0", true),
    ("foo", "foo", true),
    ("", " ", false),
    ("", "0", false),
    ("", "foo", false),
    (" ", "0", false),
    (" ", "foo", false),
    ("0", "foo", false),
];

#[test]
fn testCompareColumnComments() {
    for (comment1, comment2, equals) in COMPARE_COLUMN_COMMENTS {
        let column1 = Column::editor()
            .set_unquoted_name("foo")
            .set_type_name(Types::INTEGER)
            .set_comment(comment1)
            .create()
            .unwrap();

        let column2 = column1
            .edit()
            .unwrap()
            .set_comment(comment2)
            .create()
            .unwrap();

        let diff1 = ColumnDiff::new(column2.clone(), column1.clone());
        let diff2 = ColumnDiff::new(column1, column2);

        assert_eq!(
            !equals,
            diff1.has_comment_changed(),
            "{comment1:?} {comment2:?}"
        );
        assert_eq!(
            !equals,
            diff2.has_comment_changed(),
            "{comment1:?} {comment2:?}"
        );
    }
}
