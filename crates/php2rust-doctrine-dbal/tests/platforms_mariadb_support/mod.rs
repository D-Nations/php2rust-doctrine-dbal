//! Port of tests/Platforms/MariaDBPlatformTest.php's own tests at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, for
//! `MariaDBPlatformTest` and the tests that extend it
//! (`MariaDB1052PlatformTest`, `MariaDB110700PlatformTest`,
//! `MariaDB120300PlatformTest`). Each test is a generic function that a
//! test file calls from a `#[test]` of the same name.
//!
//! Upstream: doctrine/dbal tests/Platforms/MariaDBPlatformTest.php
#![allow(
    dead_code,
    non_snake_case,
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use php_value::PhpArray;
use php2rust_doctrine_dbal::platforms::AbstractPlatform;
use php2rust_doctrine_dbal::types::Types;

use crate::platforms_mysql_support::MySQLPlatformTestCase;
use crate::platforms_support::sql;

/// From `MariaDB` 10.2.7, JSON type is an alias to LONGTEXT however from
/// 10.4.3 setting a column as JSON adds additional functionality so use
/// JSON.
pub fn testReturnsJsonTypeDeclarationSQL<T: MySQLPlatformTestCase>() {
    assert_eq!(
        T::create_platform()
            .get_json_type_declaration_sql(&PhpArray::new())
            .unwrap(),
        sql("JSON")
    );
}

pub fn testInitializesJsonTypeMapping<T: MySQLPlatformTestCase>() {
    let platform = T::create_platform();
    assert!(platform.has_doctrine_type_mapping_for("json").unwrap());
    assert_eq!(
        platform.get_doctrine_type_mapping("json").unwrap(),
        Types::JSON
    );
}
