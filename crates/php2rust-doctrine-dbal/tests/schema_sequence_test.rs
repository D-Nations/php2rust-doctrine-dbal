//! Port of tests/Schema/SequenceTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Schema/SequenceTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php2rust_doctrine_dbal::schema::name::Identifier;
use php2rust_doctrine_dbal::schema::{Column, PrimaryKeyConstraint, Sequence, Table};
use php2rust_doctrine_dbal::types::Types;
use schema_support::expect_deprecation;

fn autoincrement_table(column_name: &str) -> Table {
    Table::editor()
        .set_unquoted_name("foo", None)
        .set_columns(vec![
            Column::editor()
                .set_unquoted_name(column_name)
                .set_type_name(Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(Some(
            PrimaryKeyConstraint::editor()
                .set_unquoted_column_names(&[column_name])
                .create()
                .unwrap(),
        ))
        .create()
        .unwrap()
}

fn sequence(name: &str, qualifier: Option<&str>) -> Sequence {
    Sequence::editor()
        .set_unquoted_name(name, qualifier)
        .create()
        .unwrap()
}

#[test]
fn testIsAutoincrementFor() {
    let table = autoincrement_table("id");

    let sequence1 = sequence("foo_id_seq", None);
    let sequence2 = sequence("bar_id_seq", None);
    let sequence3 = sequence("foo_id_seq", Some("other"));

    assert!(sequence1.is_auto_increments_for(&table).unwrap());
    assert!(!sequence2.is_auto_increments_for(&table).unwrap());
    assert!(!sequence3.is_auto_increments_for(&table).unwrap());
}

#[test]
fn testIsAutoincrementForCaseInsensitive() {
    let table = autoincrement_table("ID");

    let sequence0 = sequence("foo_id_seq", None);
    let sequence1 = sequence("foo_ID_seq", None);
    let sequence2 = sequence("bar_id_seq", None);
    let sequence3 = sequence("bar_ID_seq", None);
    let sequence4 = sequence("foo_id_seq", Some("other"));

    assert!(sequence0.is_auto_increments_for(&table).unwrap());
    assert!(sequence1.is_auto_increments_for(&table).unwrap());
    assert!(!sequence2.is_auto_increments_for(&table).unwrap());
    assert!(!sequence3.is_auto_increments_for(&table).unwrap());
    assert!(!sequence4.is_auto_increments_for(&table).unwrap());
}

#[test]
fn testEmptyName() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6646", || {
        Sequence::new("");
    });
}

#[test]
fn testGetUnqualifiedObjectName() {
    let sequence = Sequence::new("user_id_seq");
    let name = sequence.get_object_name().unwrap();

    assert_eq!(
        &Identifier::unquoted("user_id_seq").unwrap(),
        name.get_unqualified_name()
    );
    assert!(name.get_qualifier().is_none());
}

#[test]
fn testGetQualifiedObjectName() {
    let sequence = Sequence::new("auth.user_id_seq");
    let name = sequence.get_object_name().unwrap();

    assert_eq!(
        &Identifier::unquoted("user_id_seq").unwrap(),
        name.get_unqualified_name()
    );
    assert_eq!(
        Some(&Identifier::unquoted("auth").unwrap()),
        name.get_qualifier()
    );
}
