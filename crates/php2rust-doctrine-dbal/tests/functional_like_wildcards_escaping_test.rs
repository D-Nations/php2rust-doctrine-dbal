//! Port of tests/Functional/LikeWildcardsEscapingTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//!
//! Upstream: doctrine/dbal tests/Functional/LikeWildcardsEscapingTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::Value;
use schema_manager_support::{platform, sqlite};

#[tokio::test]
async fn testFetchLikeExpressionResult() {
    let connection = sqlite();
    let string = "_25% off_ your next purchase \\o/ [$̲̅(̲̅5̲̅)̲̅$̲̅] (^̮^)";
    let escape_char = "!";
    let database_platform = platform(&connection).await;

    let mut statement = connection
        .prepare(&database_platform.get_dummy_select_sql(&format!(
            "(CASE WHEN '{string}' LIKE '{}' ESCAPE '{escape_char}' THEN 1 ELSE 0 END)",
            database_platform.escape_string_for_like(string, escape_char),
        )))
        .await
        .unwrap();
    let mut result = statement.execute_query().await.unwrap();

    // (bool) $result->fetchOne()
    let value = result.fetch_one().unwrap().unwrap_or(Value::Bool(false));
    assert!(value.is_truthy());
}
