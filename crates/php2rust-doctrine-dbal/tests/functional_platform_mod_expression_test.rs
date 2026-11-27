//! Port of tests/Functional/Platform/ModExpressionTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's;
//! `assertEquals()` is PHPUnit's loose comparison.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/ModExpressionTest.php
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

#[tokio::test]
async fn testModExpression() {
    let connection = sqlite();
    let platform = platform(&connection).await;
    let query = platform.get_dummy_select_sql(&platform.get_mod_expression("5", "2"));

    let actual = connection
        .fetch_one(&query, &PhpArray::new(), &ParameterTypes::new())
        .await
        .unwrap()
        .unwrap_or(Value::Bool(false));
    assert!(phpunit_equals(&Value::string("1"), &actual), "{actual:?}");
}
