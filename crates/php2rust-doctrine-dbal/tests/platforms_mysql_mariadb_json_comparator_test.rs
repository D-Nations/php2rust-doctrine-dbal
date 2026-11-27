//! Port of tests/Platforms/MySQL/MariaDBJsonComparatorTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. The provider's
//! rows are recorded (`Platforms/MySQL/MariaDBJsonComparatorTest`).
//!
//! Upstream: doctrine/dbal tests/Platforms/MySQL/MariaDBJsonComparatorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;

use std::collections::HashMap;
use std::sync::LazyLock;

use php_value::php_array;
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::platforms::MariaDBPlatform;
use php2rust_doctrine_dbal::platforms::mysql::{
    CharsetMetadataProvider, CollationMetadataProvider, Comparator, DefaultTableOptions,
};
use php2rust_doctrine_dbal::schema::{Column, ComparatorConfig, Table};
use php2rust_doctrine_dbal::types::Types;

static PLATFORM: LazyLock<MariaDBPlatform> = LazyLock::new(MariaDBPlatform::new);

struct CharsetMetadataProviderStub;

impl CharsetMetadataProvider for CharsetMetadataProviderStub {
    fn get_default_charset_collation(&self, _charset: &str) -> Result<Option<String>, Exception> {
        Ok(None)
    }
}

struct CollationMetadataProviderStub;

impl CollationMetadataProvider for CollationMetadataProviderStub {
    fn get_collation_charset(&self, _collation: &str) -> Result<Option<String>, Exception> {
        Ok(None)
    }
}

/// `setUp()`: `$this->comparator`.
fn comparator() -> Comparator<'static> {
    Comparator::new(
        &*PLATFORM,
        Box::new(CharsetMetadataProviderStub),
        Box::new(CollationMetadataProviderStub),
        DefaultTableOptions::new("utf8mb4", "utf8mb4_general_ci"),
        ComparatorConfig::default(),
    )
}

fn json_column(name: &str, collation: Option<&str>) -> Column {
    let editor = Column::editor()
        .set_unquoted_name(name)
        .set_type_name(Types::JSON);
    match collation {
        Some(collation) => editor.set_collation(Some(collation)),
        None => editor,
    }
    .create()
    .unwrap()
}

/// `setUp()`: `$this->tables`.
fn tables() -> HashMap<&'static str, Table> {
    let mut tables = HashMap::new();

    // TableA has collation set at table level and various column collations
    tables.insert(
        "A",
        Table::editor()
            .set_unquoted_name("foo", None)
            .set_columns(vec![
                json_column("json_1", Some("latin1_swedish_ci")),
                json_column("json_2", Some("utf8_general_ci")),
                json_column("json_3", None),
            ])
            .set_options(php_array![
                "charset" => "latin1",
                "collation" => "latin1_swedish_ci",
            ])
            .create()
            .unwrap(),
    );

    // TableB has no table-level collation and various column collations
    tables.insert(
        "B",
        Table::editor()
            .set_unquoted_name("foo", None)
            .set_columns(vec![
                json_column("json_1", Some("latin1_swedish_ci")),
                json_column("json_2", Some("utf8_general_ci")),
                json_column("json_3", None),
            ])
            .create()
            .unwrap(),
    );

    // Table C has no table-level collation and column collations as MariaDb would return for columns declared
    // as JSON
    tables.insert(
        "C",
        Table::editor()
            .set_unquoted_name("foo", None)
            .set_columns(vec![
                json_column("json_1", Some("utf8mb4_bin")),
                json_column("json_2", Some("utf8mb4_bin")),
                json_column("json_3", Some("utf8mb4_bin")),
            ])
            .create()
            .unwrap(),
    );

    // Table D has no table or column collations set
    tables.insert(
        "D",
        Table::editor()
            .set_unquoted_name("foo", None)
            .set_columns(vec![
                json_column("json_1", None),
                json_column("json_2", None),
                json_column("json_3", None),
            ])
            .create()
            .unwrap(),
    );

    tables
}

#[test]
fn testJsonColumnComparison() {
    let rows = common::provider(
        "Platforms/MySQL/MariaDBJsonComparatorTest",
        "testJsonColumnComparison",
    );
    assert_eq!(rows.len(), 6);
    let tables_by_name = tables();
    let comparator = comparator();
    for (name, args) in rows {
        let table1 = common::text(&args[0]);
        let table2 = common::text(&args[1]);
        assert!(
            comparator
                .compare_tables(
                    &tables_by_name[table1.as_str()],
                    &tables_by_name[table2.as_str()]
                )
                .unwrap()
                .is_empty(),
            "data set {name}: Tables {table1} and {table2} should be identical"
        );
    }
}
