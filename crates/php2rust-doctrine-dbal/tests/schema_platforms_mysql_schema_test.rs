//! Port of tests/Schema/Platforms/MySQLSchemaTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/Platforms/MySQLSchemaTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::platforms::mysql::{
    CharsetMetadataProvider, CollationMetadataProvider, Comparator, DefaultTableOptions,
};
use php2rust_doctrine_dbal::platforms::{AbstractPlatform, MySQLPlatform};
use php2rust_doctrine_dbal::schema::{
    Column, ComparatorConfig, ForeignKeyConstraint, PrimaryKeyConstraint, Table,
};
use php2rust_doctrine_dbal::types::Types;

static PLATFORM: std::sync::LazyLock<MySQLPlatform> = std::sync::LazyLock::new(MySQLPlatform::new);

/// `setUp()`: `$this->platform`.
fn platform() -> MySQLPlatform {
    MySQLPlatform::new()
}

/// `self::createStub(CharsetMetadataProvider::class)`: `null`.
struct CharsetMetadataProviderStub;

impl CharsetMetadataProvider for CharsetMetadataProviderStub {
    fn get_default_charset_collation(&self, _charset: &str) -> Result<Option<String>, Exception> {
        Ok(None)
    }
}

/// `self::createStub(CollationMetadataProvider::class)`: `null`.
struct CollationMetadataProviderStub;

impl CollationMetadataProvider for CollationMetadataProviderStub {
    fn get_collation_charset(&self, _collation: &str) -> Result<Option<String>, Exception> {
        Ok(None)
    }
}

/// `createComparator()`.
fn create_comparator() -> Comparator<'static> {
    Comparator::new(
        &*PLATFORM,
        Box::new(CharsetMetadataProviderStub),
        Box::new(CollationMetadataProviderStub),
        DefaultTableOptions::new("utf8mb4", "utf8mb4_general_ci"),
        ComparatorConfig::default(),
    )
}

#[test]
fn testGenerateForeignKeySQL() {
    let platform = platform();
    let table_old = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("foo_id")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ])
        .set_foreign_key_constraints(vec![
            ForeignKeyConstraint::editor()
                .set_unquoted_referencing_column_names(&["foo_id"])
                .set_unquoted_referenced_table_name("test_foreign", None)
                .set_unquoted_referenced_column_names(&["foo_id"])
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let mut sqls = Vec::new();
    for fk in table_old.get_foreign_keys().values() {
        sqls.push(
            platform
                .get_create_foreign_key_sql(fk, &table_old.get_quoted_name(&platform))
                .unwrap(),
        );
    }

    assert_eq!(
        sqls,
        ["ALTER TABLE test ADD FOREIGN KEY (foo_id) REFERENCES test_foreign (foo_id)"]
    );
}

#[test]
fn testClobNoAlterTable() {
    let platform = platform();
    let table_old = Table::editor()
        .set_unquoted_name("test", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("id")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
            Column::editor()
                .set_unquoted_name("description")
                .set_type_name(Types::STRING)
                .set_length(Some(65536))
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    let table_new = table_old
        .edit()
        .unwrap()
        .set_primary_key_constraint(Some(
            PrimaryKeyConstraint::editor()
                .set_unquoted_column_names(&["id"])
                .create()
                .unwrap(),
        ))
        .create()
        .unwrap();

    let diff = create_comparator()
        .compare_tables(&table_old, &table_new)
        .unwrap();

    let sql = platform.get_alter_table_sql(&diff).unwrap();

    assert_eq!(sql, ["ALTER TABLE test ADD PRIMARY KEY (id)"]);
}
