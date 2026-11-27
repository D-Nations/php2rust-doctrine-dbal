//! Port of tests/Schema/MySQLInheritCharsetTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `self::createStub(Driver::class)` is the connection tests'
//! recording driver, which no call reaches.
//!
//! Upstream: doctrine/dbal tests/Schema/MySQLInheritCharsetTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{Script, TestDriver, run};
use php_value::{Key, PhpArray, Value, php_array};
use php2rust_doctrine_dbal::platforms::{AbstractPlatform, MySQLPlatform};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Column, MySQLSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;

/// `getTableOptionsForOverride($params)`.
fn get_table_options_for_override(params: PhpArray) -> PhpArray {
    let driver_mock = TestDriver::new(Script::default());

    let platform = MySQLPlatform::new();
    let conn = driver_mock.connection(params);
    let manager = MySQLSchemaManager::new(&conn, &platform);

    let schema_config = run(manager.create_schema_config()).unwrap();

    schema_config.get_default_table_options().clone()
}

/// `isset($options['charset'])`.
fn isset_charset(options: &PhpArray) -> bool {
    options
        .get(&Key::from("charset"))
        .is_some_and(|v| !v.is_null())
}

#[test]
fn testInheritTableOptionsFromDatabase() {
    // default, no overrides
    let options = get_table_options_for_override(PhpArray::new());
    assert!(!isset_charset(&options));

    // explicit utf8
    let options = get_table_options_for_override(php_array!["charset" => "utf8"]);
    assert!(isset_charset(&options));
    assert_eq!(
        options.get(&Key::from("charset")),
        Some(&Value::string("utf8"))
    );

    // explicit utf8mb4
    let options = get_table_options_for_override(php_array!["charset" => "utf8mb4"]);
    assert!(isset_charset(&options));
    assert_eq!(
        options.get(&Key::from("charset")),
        Some(&Value::string("utf8mb4"))
    );
}

#[test]
fn testTableOptions() {
    let platform = MySQLPlatform::new();

    // no options
    let table = Table::editor()
        .set_unquoted_name("foobar", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("aa")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    assert_eq!(
        platform.get_create_table_sql(&table).unwrap(),
        ["CREATE TABLE foobar (aa INT NOT NULL)"]
    );

    // charset
    let table = Table::editor()
        .set_unquoted_name("foobar", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name("aa")
                .set_type_name(Types::INTEGER)
                .create()
                .unwrap(),
        ])
        .set_options(php_array!["charset" => "utf8"])
        .create()
        .unwrap();

    assert_eq!(
        platform.get_create_table_sql(&table).unwrap(),
        ["CREATE TABLE foobar (aa INT NOT NULL) DEFAULT CHARACTER SET utf8"]
    );
}
