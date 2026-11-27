//! Port of tests/Functional/Platform/DefaultExpressionTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's;
//! `assertEquals()` is PHPUnit's loose comparison. The `MySQL` and `Oracle`
//! skips do not apply on `SQLite`.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/DefaultExpressionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use php2rust_doctrine_dbal::schema::{
    CurrentDate, CurrentTime, CurrentTimestamp, DefaultExpression, Table,
};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, exec, platform, sqlite};

/// `assertDefaultExpression($typeName, $expression)`: `default_value` is
/// the expression as a PHP object.
async fn assert_default_expression(
    type_name: &str,
    expression: &dyn DefaultExpression,
    default_value: Value,
) {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("default_expr_test", None)
        .set_columns(vec![
            column("actual_value", type_name).create().unwrap(),
            column("default_value", type_name)
                .set_default_value(default_value)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let platform = platform(&connection).await;

    exec(
        &connection,
        &format!(
            "INSERT INTO default_expr_test (actual_value) VALUES ({})",
            expression.to_sql(platform.as_dyn())
        ),
    )
    .await;

    let row = connection
        .fetch_numeric(
            "SELECT default_value, actual_value FROM default_expr_test",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
    let row = row.expect("assertNotFalse($row)");

    let default = row.get(&Key::Int(0)).cloned().unwrap_or(Value::Null);
    let actual = row.get(&Key::Int(1)).cloned().unwrap_or(Value::Null);
    assert!(phpunit_equals(&default, &actual), "{default:?} {actual:?}");
}

#[tokio::test]
async fn testCurrentDate() {
    assert_default_expression(Types::DATE_MUTABLE, &CurrentDate, CurrentDate::value()).await;
}

#[tokio::test]
async fn testCurrentTime() {
    assert_default_expression(Types::TIME_MUTABLE, &CurrentTime, CurrentTime::value()).await;
}

#[tokio::test]
async fn testCurrentTimestamp() {
    assert_default_expression(
        Types::DATETIME_MUTABLE,
        &CurrentTimestamp,
        CurrentTimestamp::value(),
    )
    .await;
}
