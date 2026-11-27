//! Port of tests/Schema/MySQLInheritCharsetTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on DBAL's `MySQLPlatform`.
//! Names and assertions are PHPUnit's.
//!
//! `testInheritTableOptionsFromDatabase` is not ported: it needs
//! `MySQLSchemaManager`, which is not ported.
//!
//! Upstream: doctrine/dbal tests/Schema/MySQLInheritCharsetTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::php_array;
use php2rust_doctrine_dbal::platforms::{AbstractPlatform, MySQLPlatform};
use php2rust_doctrine_dbal::schema::{Column, Table};
use php2rust_doctrine_dbal::types::Types;

/// `Column::editor()->setUnquotedName('aa')->setTypeName(Types::INTEGER)->create()`.
fn column_aa() -> Column {
    Column::editor()
        .set_unquoted_name("aa")
        .set_type_name(Types::INTEGER)
        .create()
        .unwrap()
}

#[test]
fn testTableOptions() {
    let platform = MySQLPlatform::new();

    // no options
    let table = Table::editor()
        .set_unquoted_name("foobar", None)
        .set_columns(vec![column_aa()])
        .create()
        .unwrap();

    assert_eq!(
        vec!["CREATE TABLE foobar (aa INT NOT NULL)"],
        platform.get_create_table_sql(&table).unwrap()
    );

    // charset
    let table = Table::editor()
        .set_unquoted_name("foobar", None)
        .set_columns(vec![column_aa()])
        .set_options(php_array!["charset" => "utf8"])
        .create()
        .unwrap();

    assert_eq!(
        vec!["CREATE TABLE foobar (aa INT NOT NULL) DEFAULT CHARACTER SET utf8"],
        platform.get_create_table_sql(&table).unwrap()
    );
}
