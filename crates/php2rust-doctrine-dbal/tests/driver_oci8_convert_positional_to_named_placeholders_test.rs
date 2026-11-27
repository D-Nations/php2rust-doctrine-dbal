//! Port of tests/Driver/OCI8/ConvertPositionalToNamedPlaceholdersTest.php
//! at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; a data provider's cases run in one test, in
//! order.
//!
//! Upstream: doctrine/dbal tests/Driver/OCI8/ConvertPositionalToNamedPlaceholdersTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::collections::BTreeMap;

use php2rust_doctrine_dbal::driver::oci8::ConvertPositionalToNamedPlaceholders;
use php2rust_doctrine_dbal::sql::Parser;

/// `positionalToNamedPlaceholdersProvider()`: input SQL, output SQL and
/// the number of parameters (`[1 => ':param1', ...]`).
const POSITIONAL_TO_NAMED_PLACEHOLDERS_PROVIDER: [(&str, &str, i64); 10] = [
    (
        "SELECT name FROM users WHERE id = ?",
        "SELECT name FROM users WHERE id = :param1",
        1,
    ),
    (
        "SELECT name FROM users WHERE id = ? AND status = ?",
        "SELECT name FROM users WHERE id = :param1 AND status = :param2",
        2,
    ),
    (
        "UPDATE users SET name = '???', status = ?",
        "UPDATE users SET name = '???', status = :param1",
        1,
    ),
    (
        "UPDATE users SET status = ?, name = '???'",
        "UPDATE users SET status = :param1, name = '???'",
        1,
    ),
    (
        "UPDATE users SET foo = ?, name = '???', status = ?",
        "UPDATE users SET foo = :param1, name = '???', status = :param2",
        2,
    ),
    (
        "UPDATE users SET name = \"???\", status = ?",
        "UPDATE users SET name = \"???\", status = :param1",
        1,
    ),
    (
        "UPDATE users SET status = ?, name = \"???\"",
        "UPDATE users SET status = :param1, name = \"???\"",
        1,
    ),
    (
        "UPDATE users SET foo = ?, name = \"???\", status = ?",
        "UPDATE users SET foo = :param1, name = \"???\", status = :param2",
        2,
    ),
    (
        "SELECT * FROM users WHERE id = ? AND name = \"\" AND status = ?",
        "SELECT * FROM users WHERE id = :param1 AND name = \"\" AND status = :param2",
        2,
    ),
    (
        "SELECT * FROM users WHERE id = ? AND name = '' AND status = ?",
        "SELECT * FROM users WHERE id = :param1 AND name = '' AND status = :param2",
        2,
    ),
];

#[test]
fn testConvertPositionalToNamedParameters() {
    for (input_sql, expected_output_sql, parameters) in POSITIONAL_TO_NAMED_PLACEHOLDERS_PROVIDER {
        let parser = Parser::new(false);
        let mut visitor = ConvertPositionalToNamedPlaceholders::new();

        parser.parse(input_sql.as_bytes(), &mut visitor).unwrap();

        let expected_output_params_map: BTreeMap<i64, String> = (1..=parameters)
            .map(|position| (position, format!(":param{position}")))
            .collect();

        assert_eq!(
            expected_output_sql.as_bytes(),
            visitor.get_sql().as_slice(),
            "{input_sql}"
        );
        assert_eq!(
            &expected_output_params_map,
            visitor.get_parameter_map(),
            "{input_sql}"
        );
    }
}
