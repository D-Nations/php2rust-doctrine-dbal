//! Port of tests/Schema/ViewEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/ViewEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php2rust_doctrine_dbal::schema::View;
use php2rust_doctrine_dbal::schema::name::OptionallyQualifiedName;
use schema_support::expect_exception;

#[test]
fn testSetUnquotedName() {
    let view = View::editor()
        .set_unquoted_name("active_users", None)
        .set_sql("SELECT 1")
        .create()
        .unwrap();

    assert_eq!(
        OptionallyQualifiedName::unquoted("active_users", None).unwrap(),
        view.get_object_name().unwrap()
    );
}

#[test]
fn testSetQuotedName() {
    let view = View::editor()
        .set_quoted_name("active_users", None)
        .set_sql("SELECT 1")
        .create()
        .unwrap();

    assert_eq!(
        OptionallyQualifiedName::quoted("active_users", None).unwrap(),
        view.get_object_name().unwrap()
    );
}

#[test]
fn testNameNotSet() {
    let editor = View::editor().set_sql("SELECT 1");

    expect_exception("InvalidViewDefinition", editor.create());
}

#[test]
fn testSqlNotSet() {
    let editor = View::editor().set_unquoted_name("active_users", None);

    expect_exception("InvalidViewDefinition", editor.create());
}

#[test]
fn testEdit() {
    let view = View::editor()
        .set_unquoted_name("active_users", None)
        .set_sql("SELECT 1")
        .create()
        .unwrap();

    assert_eq!(view, view.edit().unwrap().create().unwrap());
}
