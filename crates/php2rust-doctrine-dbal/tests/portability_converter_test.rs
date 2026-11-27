//! Port of tests/Portability/ConverterTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order. PHP's
//! `false` (no row) is `None`. `assertSame()` of arrays is
//! `Value::identical()` (same keys in the same order, identical values).
//!
//! Upstream: doctrine/dbal tests/Portability/ConverterTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::portability::Converter;

/// A string, or `null` for `None`.
fn v(value: Option<&str>) -> Value {
    value.map_or(Value::Null, Value::string)
}

fn numeric(values: &[Option<&str>]) -> PhpArray {
    PhpArray::list(values.iter().map(|value| v(*value)))
}

fn associative(pairs: &[(&str, Option<&str>)]) -> PhpArray {
    let mut row = PhpArray::new();
    for (key, value) in pairs {
        row.insert(*key, v(*value));
    }
    row
}

fn assert_same_rows(expected: Option<&PhpArray>, actual: Option<&PhpArray>, case: &str) {
    match (expected, actual) {
        (None, None) => {}
        (Some(expected), Some(actual)) => assert!(
            Value::Array(expected.clone()).identical(&Value::Array(actual.clone())),
            "{case}: expected {expected:?}, got {actual:?}"
        ),
        _ => panic!("{case}: expected {expected:?}, got {actual:?}"),
    }
}

fn assert_same_lists(expected: &[PhpArray], actual: &[PhpArray], case: &str) {
    let list =
        |rows: &[PhpArray]| Value::Array(PhpArray::list(rows.iter().cloned().map(Value::Array)));
    assert!(
        list(expected).identical(&list(actual)),
        "{case}: expected {expected:?}, got {actual:?}"
    );
}

/// `createConverter($convertEmptyStringToNull, $rightTrimString, $case)`.
fn create_converter(
    convert_empty_string_to_null: bool,
    right_trim_string: bool,
    case: Option<i64>,
) -> Converter {
    Converter::new(convert_empty_string_to_null, right_trim_string, case).unwrap()
}

const LOWER: Option<i64> = Some(Converter::CASE_LOWER);

#[test]
fn testConvertNumeric() {
    let row = numeric(&[Some("X "), Some("")]);

    // convertNumericProvider()
    let cases: Vec<(&str, Option<PhpArray>, bool, bool, Option<PhpArray>)> = vec![
        (
            "None",
            Some(row.clone()),
            false,
            false,
            Some(numeric(&[Some("X "), Some("")])),
        ),
        (
            "Trim",
            Some(row.clone()),
            false,
            true,
            Some(numeric(&[Some("X"), Some("")])),
        ),
        (
            "Empty to NULL",
            Some(row.clone()),
            true,
            false,
            Some(numeric(&[Some("X "), None])),
        ),
        (
            "Empty to NULL and Trim",
            Some(row),
            true,
            true,
            Some(numeric(&[Some("X"), None])),
        ),
        ("False", None, true, true, None),
    ];

    for (case, row, convert_empty_string_to_null, right_trim_string, expected) in cases {
        let actual = create_converter(convert_empty_string_to_null, right_trim_string, None)
            .convert_numeric(row);
        assert_same_rows(expected.as_ref(), actual.as_ref(), case);
    }
}

#[test]
fn testConvertAssociative() {
    let row = associative(&[("FOO", Some("")), ("BAR", Some("X "))]);

    // convertAssociativeProvider()
    let cases: Vec<(
        &str,
        Option<PhpArray>,
        bool,
        bool,
        Option<i64>,
        Option<PhpArray>,
    )> = vec![
        (
            "None",
            Some(row.clone()),
            false,
            false,
            None,
            Some(associative(&[("FOO", Some("")), ("BAR", Some("X "))])),
        ),
        (
            "Trim",
            Some(row.clone()),
            false,
            true,
            None,
            Some(associative(&[("FOO", Some("")), ("BAR", Some("X"))])),
        ),
        (
            "Empty to NULL",
            Some(row.clone()),
            true,
            false,
            None,
            Some(associative(&[("FOO", None), ("BAR", Some("X "))])),
        ),
        (
            "Empty to NULL and Trim",
            Some(row.clone()),
            true,
            true,
            None,
            Some(associative(&[("FOO", None), ("BAR", Some("X"))])),
        ),
        (
            "To lower",
            Some(row.clone()),
            false,
            false,
            LOWER,
            Some(associative(&[("foo", Some("")), ("bar", Some("X "))])),
        ),
        (
            "Trim and to lower",
            Some(row.clone()),
            false,
            true,
            LOWER,
            Some(associative(&[("foo", Some("")), ("bar", Some("X"))])),
        ),
        (
            "Empty to NULL and to lower",
            Some(row.clone()),
            true,
            false,
            LOWER,
            Some(associative(&[("foo", None), ("bar", Some("X "))])),
        ),
        (
            "Trim, empty to NULL and to lower",
            Some(row),
            true,
            true,
            LOWER,
            Some(associative(&[("foo", None), ("bar", Some("X"))])),
        ),
        ("False", None, true, true, None, None),
    ];

    for (case, row, convert_empty_string_to_null, right_trim_string, column_case, expected) in cases
    {
        let actual = create_converter(convert_empty_string_to_null, right_trim_string, column_case)
            .convert_associative(row);
        assert_same_rows(expected.as_ref(), actual.as_ref(), case);
    }
}

#[test]
fn testConvertOne() {
    // convertOneProvider()
    let cases: [(&str, Option<Value>, bool, bool, Option<Value>); 9] = [
        (
            "None, trailing space",
            Some(v(Some("X "))),
            false,
            false,
            Some(v(Some("X "))),
        ),
        (
            "None, empty string",
            Some(v(Some(""))),
            false,
            false,
            Some(v(Some(""))),
        ),
        (
            "Trim, trailing space",
            Some(v(Some("X "))),
            false,
            true,
            Some(v(Some("X"))),
        ),
        (
            "Trim, empty string",
            Some(v(Some(""))),
            false,
            true,
            Some(v(Some(""))),
        ),
        (
            "Empty to NULL, trailing space",
            Some(v(Some("X "))),
            true,
            false,
            Some(v(Some("X "))),
        ),
        (
            "Empty to NULL, empty string",
            Some(v(Some(""))),
            true,
            false,
            Some(Value::Null),
        ),
        (
            "Empty to NULL and Trim, trailing space",
            Some(v(Some("X "))),
            true,
            true,
            Some(v(Some("X"))),
        ),
        (
            "Empty to NULL and Trim, empty string",
            Some(v(Some(""))),
            true,
            true,
            Some(Value::Null),
        ),
        ("False", None, true, true, None),
    ];

    for (case, value, convert_empty_string_to_null, right_trim_string, expected) in cases {
        let actual = create_converter(convert_empty_string_to_null, right_trim_string, None)
            .convert_one(value);
        match (&expected, &actual) {
            (None, None) => {}
            (Some(expected), Some(actual)) => {
                assert!(expected.identical(actual), "{case}: {actual:?}");
            }
            _ => panic!("{case}: expected {expected:?}, got {actual:?}"),
        }
    }
}

#[test]
fn testConvertAllNumeric() {
    let data = vec![
        numeric(&[Some("X "), Some("")]),
        numeric(&[Some(""), Some("Y ")]),
    ];

    // convertAllNumericProvider()
    let cases: Vec<(&str, bool, bool, Vec<PhpArray>)> = vec![
        (
            "None",
            false,
            false,
            vec![
                numeric(&[Some("X "), Some("")]),
                numeric(&[Some(""), Some("Y ")]),
            ],
        ),
        (
            "Trim",
            false,
            true,
            vec![
                numeric(&[Some("X"), Some("")]),
                numeric(&[Some(""), Some("Y")]),
            ],
        ),
        (
            "Empty to NULL",
            true,
            false,
            vec![numeric(&[Some("X "), None]), numeric(&[None, Some("Y ")])],
        ),
        (
            "Empty to NULL and Trim",
            true,
            true,
            vec![numeric(&[Some("X"), None]), numeric(&[None, Some("Y")])],
        ),
    ];

    for (case, convert_empty_string_to_null, right_trim_string, expected) in cases {
        let actual = create_converter(convert_empty_string_to_null, right_trim_string, None)
            .convert_all_numeric(data.clone());
        assert_same_lists(&expected, &actual, case);
    }
}

#[test]
fn testConvertAllAssociative() {
    let data = vec![
        associative(&[("FOO", Some("X ")), ("BAR", Some(""))]),
        associative(&[("FOO", Some("")), ("BAR", Some("Y "))]),
    ];

    // convertAllAssociativeProvider()
    let cases: Vec<(&str, bool, bool, Option<i64>, Vec<PhpArray>)> = vec![
        (
            "None",
            false,
            false,
            None,
            vec![
                associative(&[("FOO", Some("X ")), ("BAR", Some(""))]),
                associative(&[("FOO", Some("")), ("BAR", Some("Y "))]),
            ],
        ),
        (
            "Trim",
            false,
            true,
            None,
            vec![
                associative(&[("FOO", Some("X")), ("BAR", Some(""))]),
                associative(&[("FOO", Some("")), ("BAR", Some("Y"))]),
            ],
        ),
        (
            "Empty to NULL",
            true,
            false,
            None,
            vec![
                associative(&[("FOO", Some("X ")), ("BAR", None)]),
                associative(&[("FOO", None), ("BAR", Some("Y "))]),
            ],
        ),
        (
            "Empty to NULL and Trim",
            true,
            true,
            None,
            vec![
                associative(&[("FOO", Some("X")), ("BAR", None)]),
                associative(&[("FOO", None), ("BAR", Some("Y"))]),
            ],
        ),
        (
            "To lower",
            false,
            false,
            LOWER,
            vec![
                associative(&[("foo", Some("X ")), ("bar", Some(""))]),
                associative(&[("foo", Some("")), ("bar", Some("Y "))]),
            ],
        ),
        (
            "Trim and to lower",
            false,
            true,
            LOWER,
            vec![
                associative(&[("foo", Some("X")), ("bar", Some(""))]),
                associative(&[("foo", Some("")), ("bar", Some("Y"))]),
            ],
        ),
        (
            "Empty to NULL and to lower",
            true,
            false,
            LOWER,
            vec![
                associative(&[("foo", Some("X ")), ("bar", None)]),
                associative(&[("foo", None), ("bar", Some("Y "))]),
            ],
        ),
        (
            "Trim, empty to NULL and to lower",
            true,
            true,
            LOWER,
            vec![
                associative(&[("foo", Some("X")), ("bar", None)]),
                associative(&[("foo", None), ("bar", Some("Y"))]),
            ],
        ),
    ];

    for (case, convert_empty_string_to_null, right_trim_string, column_case, expected) in cases {
        let actual = create_converter(convert_empty_string_to_null, right_trim_string, column_case)
            .convert_all_associative(data.clone());
        assert_same_lists(&expected, &actual, case);
    }
}

#[test]
fn testConvertFirstColumn() {
    let column = vec![v(Some("X ")), v(Some(""))];

    // convertFirstColumnProvider()
    let cases: Vec<(&str, bool, bool, Vec<Value>)> = vec![
        ("None", false, false, vec![v(Some("X ")), v(Some(""))]),
        ("Trim", false, true, vec![v(Some("X")), v(Some(""))]),
        (
            "Empty to NULL",
            true,
            false,
            vec![v(Some("X ")), Value::Null],
        ),
        (
            "Empty to NULL and Trim",
            true,
            true,
            vec![v(Some("X")), Value::Null],
        ),
    ];

    for (case, convert_empty_string_to_null, right_trim_string, expected) in cases {
        let actual = create_converter(convert_empty_string_to_null, right_trim_string, None)
            .convert_first_column(column.clone());
        assert!(
            Value::Array(PhpArray::list(expected.clone()))
                .identical(&Value::Array(PhpArray::list(actual.clone()))),
            "{case}: expected {expected:?}, got {actual:?}"
        );
    }
}
