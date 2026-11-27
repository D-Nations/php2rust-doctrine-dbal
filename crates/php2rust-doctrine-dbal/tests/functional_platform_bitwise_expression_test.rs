//! Port of tests/Functional/Platform/BitwiseExpressionTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's;
//! `assertEquals()` is PHPUnit's loose comparison.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/BitwiseExpressionTest.php
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
use php2rust_doctrine_dbal::platforms::AbstractPlatform;
use schema_manager_support::{platform, sqlite};

/// `assertExpressionEquals($expected, $expression)`.
async fn assert_expression_equals(expected: &str, expression: fn(&dyn AbstractPlatform) -> String) {
    let connection = sqlite();
    let platform = platform(&connection).await;
    let query = platform.get_dummy_select_sql(&expression(platform.as_dyn()));

    let actual = connection
        .fetch_one(&query, &PhpArray::new(), &ParameterTypes::new())
        .await
        .unwrap()
        .unwrap_or(Value::Bool(false));
    assert!(
        phpunit_equals(&Value::string(expected), &actual),
        "{actual:?}"
    );
}

#[tokio::test]
async fn testBitwiseAnd() {
    assert_expression_equals("2", |platform| {
        platform.get_bit_and_comparison_expression("3", "6")
    })
    .await;
}

#[tokio::test]
async fn testBitwiseOr() {
    assert_expression_equals("7", |platform| {
        platform.get_bit_or_comparison_expression("3", "6")
    })
    .await;
}
