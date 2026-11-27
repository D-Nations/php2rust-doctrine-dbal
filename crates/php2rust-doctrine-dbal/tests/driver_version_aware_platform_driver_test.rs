//! Port of tests/Driver/VersionAwarePlatformDriverTest.php: the `MySQL`
//! cases on the `pdo_mysql` driver (the `pdo_mysql` cargo feature). The
//! data provider's cases run in one test, in order.
//! `testMalformedMariaDBVersionThrows()` asks the `mysqli` driver, which
//! is not ported; the method it tests is `AbstractMySQLDriver`'s, which
//! `pdo_mysql` shares, so it asks `pdo_mysql`.
//!
//! Upstream: doctrine/dbal tests/Driver/VersionAwarePlatformDriverTest.php
#![cfg(feature = "pdo_mysql")]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::driver::{Driver, pdo_mysql};
use php2rust_doctrine_dbal::{Exception, StaticServerVersionProvider};

/// `mySQLVersionProvider()`.
const MYSQL_VERSIONS: [(&str, &str); 12] = [
    ("5.7.0", "Doctrine\\DBAL\\Platforms\\MySQLPlatform"),
    ("8.0.11", "Doctrine\\DBAL\\Platforms\\MySQL80Platform"),
    ("8.4.1", "Doctrine\\DBAL\\Platforms\\MySQL84Platform"),
    ("9.0.0", "Doctrine\\DBAL\\Platforms\\MySQL84Platform"),
    (
        "5.5.40-MariaDB-1~wheezy",
        "Doctrine\\DBAL\\Platforms\\MariaDBPlatform",
    ),
    (
        "5.5.5-MariaDB-10.2.8+maria~xenial-log",
        "Doctrine\\DBAL\\Platforms\\MariaDBPlatform",
    ),
    (
        "10.2.8-MariaDB-10.2.8+maria~xenial-log",
        "Doctrine\\DBAL\\Platforms\\MariaDBPlatform",
    ),
    (
        "10.2.8-MariaDB-1~lenny-log",
        "Doctrine\\DBAL\\Platforms\\MariaDBPlatform",
    ),
    (
        "10.5.2-MariaDB-1~lenny-log",
        "Doctrine\\DBAL\\Platforms\\MariaDB1052Platform",
    ),
    (
        "10.6.0-MariaDB-1~lenny-log",
        "Doctrine\\DBAL\\Platforms\\MariaDB1060Platform",
    ),
    (
        "10.9.3-MariaDB-1~lenny-log",
        "Doctrine\\DBAL\\Platforms\\MariaDB1060Platform",
    ),
    (
        "11.0.2-MariaDB-1:11.0.2+maria~ubu2204",
        "Doctrine\\DBAL\\Platforms\\MariaDB1010Platform",
    ),
];

/// `assertDriverInstantiatesDatabasePlatform($driver, $version,
/// $expectedClass)`.
async fn assert_driver_instantiates_database_platform(
    driver: &dyn Driver,
    version: &str,
    expected_class: &str,
) {
    let platform = driver
        .get_database_platform(&StaticServerVersionProvider::new(version))
        .await
        .unwrap();
    assert_eq!(expected_class, platform.class(), "{version}");
}

#[tokio::test]
async fn testPDOMySQL() {
    for (version, expected_class) in MYSQL_VERSIONS {
        assert_driver_instantiates_database_platform(
            &pdo_mysql::Driver::new(),
            version,
            expected_class,
        )
        .await;
    }
}

#[tokio::test]
async fn testMalformedMariaDBVersionThrows() {
    let driver = pdo_mysql::Driver::new();
    let version_provider = StaticServerVersionProvider::new("mariadb-invalid");

    let e: Exception = driver
        .get_database_platform(&version_provider)
        .await
        .err()
        .unwrap();
    assert!(e.is_instance_of(Exception::INVALID_PLATFORM_VERSION), "{e}");
}
