//! Port of tests/Cache/ArrayResultTest.php. The `#[TestWith]` rows and
//! the data provider's files run in one test each, in order; the files are
//! upstream's fixtures (tests/data/cache).
//!
//! Upstream: doctrine/dbal tests/Cache/ArrayResultTest.php
#![allow(non_snake_case)]
#![allow(clippy::unwrap_used, reason = "tests")]

use php_value::{PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::cache::ArrayResult;
use php2rust_doctrine_dbal::driver::Result as _;

fn s(text: &str) -> Value {
    Value::string(text)
}

fn names(names: &[&str]) -> Vec<PhpString> {
    names.iter().map(|name| PhpString::from(*name)).collect()
}

fn assoc(pairs: &[(&str, Value)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(*key, value.clone());
    }
    array
}

fn set_up() -> ArrayResult {
    ArrayResult::new(
        names(&["username", "active"]),
        vec![
            PhpArray::list([s("jwage"), Value::Bool(true)]),
            PhpArray::list([s("romanb"), Value::Bool(false)]),
        ],
    )
}

fn both_rows() -> Vec<PhpArray> {
    vec![
        assoc(&[("username", s("jwage")), ("active", Value::Bool(true))]),
        assoc(&[("username", s("romanb")), ("active", Value::Bool(false))]),
    ]
}

#[test]
fn testFree() {
    let mut result = set_up();
    assert_eq!(result.row_count().unwrap(), 2);

    result.free();

    assert_eq!(result.row_count().unwrap(), 0);
}

#[test]
fn testColumnCount() {
    assert_eq!(set_up().column_count().unwrap(), 2);
}

#[test]
fn testColumnNames() {
    let result = set_up();
    assert_eq!(result.get_column_name(0).unwrap().unwrap(), "username");
    assert_eq!(result.get_column_name(1).unwrap().unwrap(), "active");
}

#[test]
fn testColumnNameWithInvalidIndex() {
    for index in [2, -1] {
        let e = set_up().get_column_name(index).unwrap().unwrap_err();
        assert!(e.is_instance_of(Exception::INVALID_COLUMN_INDEX));
    }
}

#[test]
fn testRowCount() {
    assert_eq!(set_up().row_count().unwrap(), 2);
}

#[test]
fn testFetchAssociative() {
    assert_eq!(
        set_up().fetch_associative().unwrap(),
        Some(assoc(&[
            ("username", s("jwage")),
            ("active", Value::Bool(true))
        ])),
    );
}

#[test]
fn testFetchNumeric() {
    assert_eq!(
        set_up().fetch_numeric().unwrap(),
        Some(PhpArray::list([s("jwage"), Value::Bool(true)])),
    );
}

#[test]
fn testFetchOne() {
    let mut result = set_up();
    assert_eq!(result.fetch_one().unwrap(), Some(s("jwage")));
    assert_eq!(result.fetch_one().unwrap(), Some(s("romanb")));
}

#[test]
fn testFetchAllAssociative() {
    assert_eq!(set_up().fetch_all_associative().unwrap(), both_rows());
}

#[test]
fn testEmptyResult() {
    let result = ArrayResult::new(names(&["a"]), vec![]);
    assert_eq!(result.get_column_name(0).unwrap().unwrap(), "a");
}

#[test]
fn testSameColumnNames() {
    let mut result = ArrayResult::new(
        names(&["a", "a"]),
        vec![PhpArray::list([Value::Int(1), Value::Int(2)])],
    );

    assert_eq!(result.get_column_name(0).unwrap().unwrap(), "a");
    assert_eq!(result.get_column_name(1).unwrap().unwrap(), "a");

    assert_eq!(
        result.fetch_numeric().unwrap(),
        Some(PhpArray::list([Value::Int(1), Value::Int(2)])),
    );
}

#[test]
fn testSerialize() {
    let mut result = ArrayResult::unserialize(set_up().serialize().as_bytes()).unwrap();

    assert_eq!(result.fetch_all_associative().unwrap(), both_rows());

    assert_eq!(result.column_count().unwrap(), 2);
    assert_eq!(result.get_column_name(0).unwrap().unwrap(), "username");
}

#[test]
fn testRowPointerIsNotSerialized() {
    let mut original = set_up();
    original.fetch_associative().unwrap();
    let mut result = ArrayResult::unserialize(original.serialize().as_bytes()).unwrap();

    assert_eq!(
        result.fetch_associative().unwrap(),
        Some(assoc(&[
            ("username", s("jwage")),
            ("active", Value::Bool(true))
        ])),
    );
}

/// `provideSerializedResultFiles()`: the 4.1 and the 4.2 format.
#[test]
fn testUnserialize() {
    for serialized in [
        include_bytes!("data/cache/array-result-4.1.txt").as_slice(),
        include_bytes!("data/cache/array-result-4.2.txt").as_slice(),
    ] {
        let mut result = ArrayResult::unserialize(serialized).unwrap();

        assert_eq!(result.fetch_all_associative().unwrap(), both_rows());

        assert_eq!(result.column_count().unwrap(), 2);
        assert_eq!(result.get_column_name(0).unwrap().unwrap(), "username");
    }
}

/// `serialize()` writes the 4.2 format, as PHP does.
#[test]
fn serialize_writes_the_4_2_format() {
    assert_eq!(
        set_up().serialize().as_bytes(),
        include_bytes!("data/cache/array-result-4.2.txt").as_slice(),
    );
}
