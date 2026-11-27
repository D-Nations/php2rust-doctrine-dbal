//! Port of tests/Driver/OCI8/ExecutionModeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Driver/OCI8/ExecutionModeTest.php
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::driver::oci8::ExecutionMode;

/// `setUp()`.
fn mode() -> ExecutionMode {
    ExecutionMode::new()
}

#[test]
fn testDefaultAutoCommitStatus() {
    assert!(mode().is_auto_commit_enabled());
}

#[test]
fn testChangeAutoCommitStatus() {
    let mut mode = mode();

    mode.disable_auto_commit();
    assert!(!mode.is_auto_commit_enabled());

    mode.enable_auto_commit();
    assert!(mode.is_auto_commit_enabled());
}
