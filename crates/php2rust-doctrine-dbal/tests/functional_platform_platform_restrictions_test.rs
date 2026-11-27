//! Port of tests/Functional/Platform/PlatformRestrictionsTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/PlatformRestrictionsTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{
    column, drop_and_create_table, platform, primary_key, schema_manager, sqlite,
};

/// Tests element names that are at the boundary of the identifier length limit.
/// Ensures generated auto-increment identifier name respects to platform restrictions.
#[tokio::test]
async fn testMaxIdentifierLengthLimitWithAutoIncrement() {
    let connection = sqlite();
    let platform = platform(&connection).await;
    let length = usize::try_from(platform.get_max_identifier_length()).unwrap();
    let table_name = "x".repeat(length);
    let column_name = "y".repeat(length);

    let table = Table::editor()
        .set_unquoted_name(&table_name, None)
        .set_columns(vec![
            column(&column_name, Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&[&column_name]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    let created_table = schema_manager(&connection)
        .await
        .introspect_table_by_unquoted_name(&table_name, None)
        .await
        .unwrap();

    assert!(created_table.has_column(&column_name));
    assert!(created_table.get_primary_key().is_some());
}
