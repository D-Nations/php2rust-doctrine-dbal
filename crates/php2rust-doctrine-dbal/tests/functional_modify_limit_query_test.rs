//! Port of tests/Functional/ModifyLimitQueryTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory,
//! with `SQLitePlatform::modifyLimitQuery()`. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Functional/ModifyLimitQueryTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, platform, primary_key, sqlite};

async fn set_up() -> Connection {
    let connection = sqlite();

    let table = Table::editor()
        .set_unquoted_name("modify_limit_table", None)
        .set_columns(vec![column("test_int", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["test_int"]))
        .create()
        .unwrap();

    let table2 = Table::editor()
        .set_unquoted_name("modify_limit_table2", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("test_int", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    drop_and_create_table(&connection, &table2).await;

    connection
}

/// `$this->connection->insert($table, ['test_int' => $value])`.
async fn insert(connection: &Connection, table: &str, value: i64) {
    let mut data = PhpArray::new();
    data.insert("test_int", Value::Int(value));
    connection
        .insert(table, &data, &ParameterTypes::new())
        .await
        .unwrap();
}

/// `assertLimitResult()`.
async fn assert_limit_result(
    connection: &Connection,
    expected_results: &[i64],
    sql: &str,
    limit: Option<i64>,
    offset: i64,
    deterministic: bool,
) {
    let p = platform(connection).await;
    let mut data = Vec::new();
    for row in connection
        .fetch_all_associative(
            &p.modify_limit_query(sql, limit, offset).unwrap(),
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
    {
        // array_change_key_case($row, CASE_LOWER)
        let value = row
            .iter()
            .find(|(key, _)| match key {
                Key::Str(name) => name.as_bytes().eq_ignore_ascii_case(b"test_int"),
                Key::Int(_) => false,
            })
            .map(|(_, value)| value.clone())
            .unwrap();
        data.push(value);
    }

    // Do not assert the order of results when results are non-deterministic
    if deterministic {
        let expected = Value::Array(PhpArray::list(
            expected_results.iter().map(|v| Value::Int(*v)),
        ));
        let actual = Value::Array(PhpArray::list(data));
        assert!(
            phpunit_equals(&expected, &actual),
            "expected {expected:?}, got {actual:?}"
        );
    } else {
        assert_eq!(expected_results.len(), data.len());
    }
}

#[tokio::test]
async fn testModifyLimitQuerySimpleQuery() {
    let connection = set_up().await;
    for value in 1..=4 {
        insert(&connection, "modify_limit_table", value).await;
    }

    let sql = "SELECT * FROM modify_limit_table ORDER BY test_int ASC";

    assert_limit_result(&connection, &[1, 2, 3, 4], sql, Some(10), 0, true).await;
    assert_limit_result(&connection, &[1, 2], sql, Some(2), 0, true).await;
    assert_limit_result(&connection, &[3, 4], sql, Some(2), 2, true).await;
    assert_limit_result(&connection, &[2, 3, 4], sql, None, 1, true).await;
}

#[tokio::test]
async fn testModifyLimitQueryJoinQuery() {
    let connection = set_up().await;
    insert(&connection, "modify_limit_table", 1).await;
    insert(&connection, "modify_limit_table", 2).await;

    for value in [1, 1, 1, 2, 2] {
        insert(&connection, "modify_limit_table2", value).await;
    }

    let sql = "SELECT modify_limit_table.test_int \
               FROM modify_limit_table \
               INNER JOIN modify_limit_table2 \
               ON modify_limit_table.test_int = modify_limit_table2.test_int \
               ORDER BY modify_limit_table.test_int DESC";

    assert_limit_result(&connection, &[2, 2, 1, 1, 1], sql, Some(10), 0, true).await;
    assert_limit_result(&connection, &[1, 1, 1], sql, Some(3), 2, true).await;
    assert_limit_result(&connection, &[2, 2], sql, Some(2), 0, true).await;
}

#[tokio::test]
async fn testModifyLimitQueryNonDeterministic() {
    let connection = set_up().await;
    for value in 1..=4 {
        insert(&connection, "modify_limit_table", value).await;
    }

    let sql = "SELECT * FROM modify_limit_table";

    assert_limit_result(&connection, &[4, 3, 2, 1], sql, Some(10), 0, false).await;
    assert_limit_result(&connection, &[4, 3], sql, Some(2), 0, false).await;
    assert_limit_result(&connection, &[2, 1], sql, Some(2), 2, false).await;
}

#[tokio::test]
async fn testModifyLimitQueryGroupBy() {
    let connection = set_up().await;
    insert(&connection, "modify_limit_table", 1).await;
    insert(&connection, "modify_limit_table", 2).await;

    for value in [1, 1, 1, 2, 2] {
        insert(&connection, "modify_limit_table2", value).await;
    }

    let sql = "SELECT modify_limit_table.test_int FROM modify_limit_table \
               INNER JOIN modify_limit_table2 ON modify_limit_table.test_int = modify_limit_table2.test_int \
               GROUP BY modify_limit_table.test_int \
               ORDER BY modify_limit_table.test_int ASC";
    assert_limit_result(&connection, &[1, 2], sql, Some(10), 0, true).await;
    assert_limit_result(&connection, &[1], sql, Some(1), 0, true).await;
    assert_limit_result(&connection, &[2], sql, Some(1), 1, true).await;
}

#[tokio::test]
async fn testModifyLimitQuerySubSelect() {
    let connection = set_up().await;
    for value in 1..=4 {
        insert(&connection, "modify_limit_table", value).await;
    }

    let sql = "SELECT modify_limit_table.*, (SELECT COUNT(*) FROM modify_limit_table) AS cnt \
               FROM modify_limit_table ORDER BY test_int DESC";

    assert_limit_result(&connection, &[4, 3, 2, 1], sql, Some(10), 0, true).await;
    assert_limit_result(&connection, &[4, 3], sql, Some(2), 0, true).await;
    assert_limit_result(&connection, &[2, 1], sql, Some(2), 2, true).await;
}

#[tokio::test]
async fn testModifyLimitQueryFromSubSelect() {
    let connection = set_up().await;
    for value in 1..=4 {
        insert(&connection, "modify_limit_table", value).await;
    }

    let sql = "SELECT * FROM (SELECT * FROM modify_limit_table) sub ORDER BY test_int DESC";

    assert_limit_result(&connection, &[4, 3, 2, 1], sql, Some(10), 0, true).await;
    assert_limit_result(&connection, &[4, 3], sql, Some(2), 0, true).await;
    assert_limit_result(&connection, &[2, 1], sql, Some(2), 2, true).await;
}

#[tokio::test]
async fn testModifyLimitQueryLineBreaks() {
    let connection = set_up().await;
    for value in 1..=3 {
        insert(&connection, "modify_limit_table", value).await;
    }

    let sql = "SELECT\n*\nFROM\nmodify_limit_table\nORDER\nBY\ntest_int\nASC";

    assert_limit_result(&connection, &[2], sql, Some(1), 1, true).await;
}

#[tokio::test]
async fn testModifyLimitQueryZeroOffsetNoLimit() {
    let connection = set_up().await;
    insert(&connection, "modify_limit_table", 1).await;
    insert(&connection, "modify_limit_table", 2).await;

    let sql = "SELECT test_int FROM modify_limit_table ORDER BY test_int ASC";

    assert_limit_result(&connection, &[1, 2], sql, None, 0, true).await;
}

#[tokio::test]
async fn testLimitWhenOrderByWithSubqueryWithOrderBy() {
    let connection = set_up().await;
    // SQLitePlatform: neither Db2, Oracle nor SQL Server.
    insert(&connection, "modify_limit_table2", 3).await;
    insert(&connection, "modify_limit_table2", 1).await;
    insert(&connection, "modify_limit_table2", 2).await;

    let subquery =
        "SELECT test_int FROM modify_limit_table2 T2 WHERE T1.id=T2.id ORDER BY test_int";

    let sql = format!("SELECT test_int FROM modify_limit_table2 T1 ORDER BY ({subquery}) ASC");

    assert_limit_result(&connection, &[1, 2, 3], &sql, Some(10), 0, true).await;
}
