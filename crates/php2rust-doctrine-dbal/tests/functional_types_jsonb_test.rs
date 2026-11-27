//! Port of tests/Functional/Types/JsonbTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/JsonbTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod comparator_test_utils;
mod schema_manager_support;

use comparator_test_utils::{diff_from_actual_to_desired_table, diff_from_desired_to_actual_table};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

#[tokio::test]
async fn testJsonbColumnIntrospection() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("test_jsonb", None)
        .set_columns(vec![column("v", Types::JSONB).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let schema_manager = schema_manager(&connection).await;
    let comparator = schema_manager.create_comparator();

    assert!(
        diff_from_actual_to_desired_table(&schema_manager, &comparator, &table)
            .await
            .unwrap()
            .is_empty()
    );

    assert!(
        diff_from_desired_to_actual_table(&schema_manager, &comparator, &table)
            .await
            .unwrap()
            .is_empty()
    );
}
