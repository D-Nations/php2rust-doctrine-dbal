//! Port of tests/Functional/Platform/LengthExpressionTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's;
//! `assertEquals()` is PHPUnit's loose comparison, and a data provider's
//! cases run in one test, in order. The SQL Server version check does not
//! apply on `SQLite`.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/LengthExpressionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use schema_manager_support::{platform, sqlite};

/// `expressionProvider()`: value, expected length, multibyte.
const EXPRESSION_PROVIDER: [(&str, &str, i64, bool); 4] = [
    ("1-byte", "Hello, world!", 13, false),
    ("2-byte", "Привет, мир!", 12, true),
    ("3-byte", "你好，世界", 5, true),
    ("4-byte", "💩", 1, true),
];

#[tokio::test]
async fn testLengthExpression() {
    for (case, value, expected, _is_multibyte) in EXPRESSION_PROVIDER {
        let connection = sqlite();
        let platform = platform(&connection).await;
        let query = platform.get_dummy_select_sql(&platform.get_length_expression("?"));

        let actual = connection
            .fetch_one(
                &query,
                &PhpArray::list([Value::string(value)]),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        assert!(
            phpunit_equals(&Value::Int(expected), &actual),
            "{case}: {actual:?}"
        );
    }
}
