//! Port of the `PostgreSQL` test of tests/Driver/VersionAwarePlatformDriverTest.php
//! at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879:
//! `testPDOPgSQL` with `postgreSQLVersionProvider()` (its cases run in
//! one test). The `pgsql` driver of `testPgSQL` is not ported.
//!
//! Upstream: doctrine/dbal tests/Driver/VersionAwarePlatformDriverTest.php
#![cfg(feature = "pdo_pgsql")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::StaticServerVersionProvider;
use php2rust_doctrine_dbal::driver::Driver as _;
use php2rust_doctrine_dbal::driver::pdo_pgsql::Driver;
use php2rust_doctrine_dbal::platforms::{PostgreSQL120Platform, PostgreSQLPlatform};

/// `postgreSQLVersionProvider()`.
const POSTGRESQL_VERSIONS: [(&str, &str); 5] = [
    ("10.0", PostgreSQLPlatform::CLASS),
    ("11.0", PostgreSQLPlatform::CLASS),
    ("12.0", PostgreSQL120Platform::CLASS),
    ("13.16", PostgreSQL120Platform::CLASS),
    ("16.4", PostgreSQL120Platform::CLASS),
];

/// `assertDriverInstantiatesDatabasePlatform()`.
async fn assert_driver_instantiates_database_platform(version: &str, expected_class: &str) {
    let platform = Driver::new()
        .get_database_platform(&StaticServerVersionProvider::new(version.to_owned()))
        .await
        .unwrap();

    assert_eq!(platform.class(), expected_class, "version {version}");
}

#[tokio::test]
async fn testPDOPgSQL() {
    for (version, expected_class) in POSTGRESQL_VERSIONS {
        assert_driver_instantiates_database_platform(version, expected_class).await;
    }
}
