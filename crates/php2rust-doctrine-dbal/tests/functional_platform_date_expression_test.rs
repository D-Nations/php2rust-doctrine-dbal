//! Port of tests/Functional/Platform/DateExpressionTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's;
//! `assertEquals()` is PHPUnit's loose comparison, and a data provider's
//! cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/DateExpressionTest.php
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
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, platform, sqlite};

/// `differenceProvider()`.
const DIFFERENCE_PROVIDER: [(&str, &str, &str, i64); 2] = [
    ("same day", "2018-04-14 23:59:59", "2018-04-14 00:00:00", 0),
    ("midnight", "2018-04-14 00:00:00", "2018-04-13 23:59:59", 1),
];

#[tokio::test]
async fn testDifference() {
    for (case, date1, date2, expected) in DIFFERENCE_PROVIDER {
        let connection = sqlite();
        let table = Table::editor()
            .set_unquoted_name("date_expr_test", None)
            .set_columns(vec![
                column("date1", Types::DATETIME_MUTABLE).create().unwrap(),
                column("date2", Types::DATETIME_MUTABLE).create().unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;
        let mut data = PhpArray::new();
        data.insert("date1", Value::string(date1));
        data.insert("date2", Value::string(date2));
        connection
            .insert("date_expr_test", &data, &ParameterTypes::new())
            .await
            .unwrap();

        let platform = platform(&connection).await;

        let sql = format!(
            "SELECT {} FROM date_expr_test",
            platform.get_date_diff_expression("date1", "date2").unwrap()
        );
        let diff = connection
            .fetch_one(&sql, &PhpArray::new(), &ParameterTypes::new())
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));

        assert!(
            phpunit_equals(&Value::Int(expected), &diff),
            "{case}: {diff:?}"
        );
    }
}
