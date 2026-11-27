//! Port of tests/Functional/Platform/ConcatExpressionTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's;
//! `assertEquals()` is PHPUnit's loose comparison, and a data provider's
//! cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/ConcatExpressionTest.php
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

/// `expressionProvider()`.
const EXPRESSION_PROVIDER: [(&str, &[&str], &str); 2] = [
    ("strings", &["'foo'", "'bar'"], "foobar"),
    (
        "numbers and a hyphen",
        &["2010", "'-'", "2019"],
        "2010-2019",
    ),
];

#[tokio::test]
async fn testConcatExpression() {
    for (case, arguments, expected) in EXPRESSION_PROVIDER {
        let connection = sqlite();
        let platform = platform(&connection).await;
        let query = platform.get_dummy_select_sql(&platform.get_concat_expression(arguments));

        let actual = connection
            .fetch_one(&query, &PhpArray::new(), &ParameterTypes::new())
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        assert!(
            phpunit_equals(&Value::string(expected), &actual),
            "{case}: {actual:?}"
        );
    }
}
