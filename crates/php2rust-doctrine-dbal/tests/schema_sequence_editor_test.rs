//! Port of tests/Schema/SequenceEditorTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. `SequenceEditor` keeps the error of a setter and returns
//! it from `create()` (PHP throws from the setter).
//!
//! Upstream: doctrine/dbal tests/Schema/SequenceEditorTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php2rust_doctrine_dbal::schema::Sequence;
use php2rust_doctrine_dbal::schema::name::OptionallyQualifiedName;
use schema_support::expect_exception;

#[test]
fn testNameNotSet() {
    let editor = Sequence::editor();

    expect_exception("InvalidSequenceDefinition", editor.create());
}

#[test]
fn testSetUnquotedName() {
    let sequence = Sequence::editor()
        .set_unquoted_name("user_id_seq", None)
        .create()
        .unwrap();

    assert_eq!(
        OptionallyQualifiedName::unquoted("user_id_seq", None).unwrap(),
        sequence.get_object_name().unwrap()
    );
}

#[test]
fn testSetQuotedName() {
    let sequence = Sequence::editor()
        .set_quoted_name("user_id_seq", None)
        .create()
        .unwrap();

    assert_eq!(
        OptionallyQualifiedName::quoted("user_id_seq", None).unwrap(),
        sequence.get_object_name().unwrap()
    );
}

#[test]
fn testNegativeCacheSize() {
    let editor = Sequence::editor();

    expect_exception(
        "InvalidSequenceDefinition",
        editor.set_cache_size(Some(-1)).create(),
    );
}

#[test]
fn testEdit() {
    let sequence = Sequence::editor()
        .set_unquoted_name("user_id_seq", None)
        .set_allocation_size(20)
        .set_initial_value(100)
        .set_cache_size(Some(5))
        .create()
        .unwrap();

    assert_eq!(sequence, sequence.edit().unwrap().create().unwrap());
}
