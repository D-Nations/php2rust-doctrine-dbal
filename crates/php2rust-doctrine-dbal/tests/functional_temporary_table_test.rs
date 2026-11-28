//! Port of tests/Functional/TemporaryTableTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Functional/TemporaryTableTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, exec, platform, primary_key, sqlite};

/// The `CREATE TEMPORARY TABLE` statement of both tests.
async fn create_temp_table_sql(connection: &Connection) -> String {
    let platform = platform(connection).await;
    let column = column("id", Types::INTEGER).create().unwrap();

    let temp_table = platform.get_temporary_table_name("my_temporary");

    format!(
        "{} {temp_table} ({})",
        platform.get_create_temporary_table_snippet_sql(),
        platform
            .get_column_declaration_list_sql(&[column.to_array(false).unwrap()])
            .unwrap(),
    )
}

async fn create_nontemporary(connection: &Connection) {
    let table = Table::editor()
        .set_unquoted_name("nontemporary", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(connection, &table).await;
}

async fn insert(connection: &Connection, id: i64) {
    let mut data = PhpArray::new();
    data.insert("id", Value::Int(id));
    connection
        .insert("nontemporary", &data, &ParameterTypes::new())
        .await
        .unwrap();
}

/// `dropTemporaryTable($name)`: an `Exception` is ignored.
async fn drop_temporary_table(connection: &Connection, name: &str) {
    let platform = platform(connection).await;
    let sql = platform.get_drop_temporary_table_sql(&platform.get_temporary_table_name(name));

    let _ = connection
        .execute_statement(&sql, &PhpArray::new(), &ParameterTypes::new())
        .await;
}

async fn assert_nontemporary_is_empty(connection: &Connection) {
    // In an event of an error this result has one row, because of an implicit commit
    assert_eq!(
        connection
            .fetch_all_associative(
                "SELECT * FROM nontemporary",
                &PhpArray::new(),
                &ParameterTypes::new()
            )
            .await
            .unwrap()
            .len(),
        0
    );
}

#[tokio::test]
async fn testDropTemporaryTableNotAutoCommitTransaction() {
    let connection = sqlite();
    // SQLitePlatform, not Oracle.
    let create_temp_table_sql = create_temp_table_sql(&connection).await;
    exec(&connection, &create_temp_table_sql).await;

    create_nontemporary(&connection).await;

    connection.begin_transaction().await.unwrap();
    insert(&connection, 1).await;
    drop_temporary_table(&connection, "my_temporary").await;
    insert(&connection, 2).await;
    connection.roll_back().await.unwrap();

    assert_nontemporary_is_empty(&connection).await;
}

#[tokio::test]
async fn testCreateTemporaryTableNotAutoCommitTransaction() {
    let connection = sqlite();
    // SQLitePlatform, not Oracle.
    let create_temp_table_sql = create_temp_table_sql(&connection).await;

    create_nontemporary(&connection).await;

    connection.begin_transaction().await.unwrap();
    insert(&connection, 1).await;
    drop_temporary_table(&connection, "my_temporary").await;
    exec(&connection, &create_temp_table_sql).await;
    insert(&connection, 2).await;
    connection.roll_back().await.unwrap();

    let platform = platform(&connection).await;
    let temp_table = platform.get_temporary_table_name("my_temporary");
    let _ = connection
        .execute_statement(
            &platform.get_drop_temporary_table_sql(&temp_table),
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await;

    assert_nontemporary_is_empty(&connection).await;
}
