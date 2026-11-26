//! Port of tests/Schema/ViewTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/ViewTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php2rust_doctrine_dbal::schema::View;
use php2rust_doctrine_dbal::schema::name::Identifier;
use schema_support::expect_deprecation;

#[test]
fn testEmptyName() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6646", || {
        View::new("", "");
    });
}

#[test]
fn testGetUnqualifiedObjectName() {
    let view = View::new("active_users", "");
    let name = view.get_object_name().unwrap();

    assert_eq!(
        &Identifier::unquoted("active_users").unwrap(),
        name.get_unqualified_name()
    );
    assert!(name.get_qualifier().is_none());
}

#[test]
fn testGetQualifiedObjectName() {
    let view = View::new("inventory.available_products", "");
    let name = view.get_object_name().unwrap();

    assert_eq!(
        &Identifier::unquoted("available_products").unwrap(),
        name.get_unqualified_name()
    );
    assert_eq!(
        Some(&Identifier::unquoted("inventory").unwrap()),
        name.get_qualifier()
    );
}
