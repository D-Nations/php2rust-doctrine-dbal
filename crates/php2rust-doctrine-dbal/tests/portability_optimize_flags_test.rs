//! Port of tests/Portability/OptimizeFlagsTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Portability/OptimizeFlagsTest.php
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::platforms::{OraclePlatform, SQLitePlatform};
use php2rust_doctrine_dbal::portability::{Connection, OptimizeFlags};

/// `setUp()`.
fn optimize_flags() -> OptimizeFlags {
    OptimizeFlags::new()
}

#[test]
fn testOracle() {
    let flags = optimize_flags().invoke(&OraclePlatform::new(), Connection::PORTABILITY_ALL);

    assert_eq!(0, flags & Connection::PORTABILITY_EMPTY_TO_NULL);
}

#[test]
fn testAnotherPlatform() {
    let flags = optimize_flags().invoke(&SQLitePlatform::new(), Connection::PORTABILITY_ALL);

    assert_eq!(Connection::PORTABILITY_ALL, flags);
}
