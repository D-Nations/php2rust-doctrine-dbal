//! The `pdo_pgsql` driver on a live `PostgreSQL` server, beyond the
//! upstream functional tests ported elsewhere: the platform for the
//! server version, values as `pdo_pgsql` gives them (booleans, integers and
//! floats typed, the rest strings), positional and named parameters,
//! the exception converter, transactions, `lastInsertId()` and the
//! schema manager's introspection of a table it created. Needs the
//! `pdo_pgsql` cargo feature and `DBAL_TEST_PGSQL_URL`; without the
//! variable each test passes with a note.
//!
//! Upstream: none
#![cfg(feature = "pdo_pgsql")]
#![allow(
    clippy::unwrap_used,
    reason = "test helpers; a panic is a test failure"
)]

mod pgsql_support;
mod schema_manager_support;

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Exception, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, exec, platform, schema_manager};

fn get(row: &PhpArray, key: &str) -> Value {
    row.get(&Key::from(key)).cloned().unwrap_or(Value::Null)
}

#[tokio::test]
async fn platform_and_server_version() {
    let Some(connection) = pgsql_support::pgsql("platform_and_server_version").await else {
        return;
    };
    let version = connection.get_server_version().await.unwrap();
    assert!(
        version.chars().next().is_some_and(|c| c.is_ascii_digit()),
        "{version}"
    );
    let class = platform(&connection).await.class().to_owned();
    assert!(
        class.starts_with("Doctrine\\DBAL\\Platforms\\PostgreSQL"),
        "{class}"
    );
}

#[tokio::test]
async fn values_and_parameters() {
    let Some(connection) = pgsql_support::pgsql("values_and_parameters").await else {
        return;
    };
    let row = connection
        .fetch_associative(
            "SELECT 1::int4 AS i, 2::int8 AS b, 1.5::float8 AS f, true AS t, \
             'x'::text AS s, 1.50::numeric AS n, NULL::text AS z, ?::text AS p",
            &PhpArray::list([Value::string("param")]),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(get(&row, "i").identical(&Value::Int(1)));
    assert!(get(&row, "b").identical(&Value::Int(2)));
    assert!(get(&row, "f").identical(&Value::Float(1.5)));
    assert!(get(&row, "t").identical(&Value::Bool(true)));
    assert!(get(&row, "s").identical(&Value::string("x")));
    assert!(get(&row, "n").identical(&Value::string("1.50")));
    assert!(get(&row, "z").is_null());
    assert!(get(&row, "p").identical(&Value::string("param")));

    let mut named = PhpArray::new();
    named.insert("a", Value::Int(40));
    named.insert("b", Value::Int(2));
    let sum = connection
        .fetch_one(
            "SELECT CAST(:a AS int) + CAST(:b AS int) + CAST(:a AS int) - 40",
            &named,
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
    assert!(sum.is_some_and(|v| v.identical(&Value::Int(42))));
}

#[tokio::test]
async fn errors_are_converted() {
    let Some(connection) = pgsql_support::pgsql("errors_are_converted").await else {
        return;
    };
    let e = connection
        .fetch_one(
            "SELECT * FROM no_such_table_here",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap_err();
    assert!(
        e.is_instance_of(Exception::TABLE_NOT_FOUND_EXCEPTION),
        "{e}"
    );
    let message = e.message().to_string_lossy().into_owned();
    assert!(message.contains("SQLSTATE[42P01]"), "{message}");

    exec(&connection, "DROP TABLE IF EXISTS pgsql_live_unique").await;
    exec(
        &connection,
        "CREATE TABLE pgsql_live_unique (id INT PRIMARY KEY)",
    )
    .await;
    exec(&connection, "INSERT INTO pgsql_live_unique VALUES (1)").await;
    let e = connection
        .execute_statement(
            "INSERT INTO pgsql_live_unique VALUES (1)",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap_err();
    assert!(
        e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION),
        "{e}"
    );
}

#[tokio::test]
async fn transactions_and_last_insert_id() {
    let Some(connection) = pgsql_support::pgsql("transactions_and_last_insert_id").await else {
        return;
    };
    exec(&connection, "DROP TABLE IF EXISTS pgsql_live_tx").await;
    exec(
        &connection,
        "CREATE TABLE pgsql_live_tx (id SERIAL PRIMARY KEY, v TEXT)",
    )
    .await;

    connection.begin_transaction().await.unwrap();
    exec(&connection, "INSERT INTO pgsql_live_tx (v) VALUES ('a')").await;
    connection.roll_back().await.unwrap();

    connection.begin_transaction().await.unwrap();
    let affected = connection
        .execute_statement(
            "INSERT INTO pgsql_live_tx (v) VALUES (?), (?)",
            &PhpArray::list([Value::string("b"), Value::string("c")]),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
    assert_eq!(affected, 2);
    let id = connection.last_insert_id().await.unwrap();
    connection.commit().await.unwrap();

    // The rolled back row took id 1.
    assert!(id.identical(&Value::string("3")), "{id:?}");
    let count = connection
        .fetch_one(
            "SELECT COUNT(*) FROM pgsql_live_tx",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
    assert!(count.is_some_and(|v| v.identical(&Value::Int(2))));
}

#[tokio::test]
async fn schema_manager_round_trip() {
    let Some(connection) = pgsql_support::pgsql("schema_manager_round_trip").await else {
        return;
    };
    let table = Table::editor()
        .set_unquoted_name("pgsql_live_schema", None)
        .set_columns(vec![
            column("id", Types::INTEGER)
                .set_autoincrement(true)
                .create()
                .unwrap(),
            column("name", Types::STRING)
                .set_length(Some(100))
                .set_default_value(Value::string("it's"))
                .create()
                .unwrap(),
            column("price", Types::DECIMAL)
                .set_precision(Some(10))
                .set_scale(2)
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(schema_manager_support::primary_key(&["id"]))
        .create()
        .unwrap();
    drop_and_create_table(&connection, &table).await;

    let sm = schema_manager(&connection).await;
    let introspected = sm
        .introspect_table_by_unquoted_name("pgsql_live_schema", None)
        .await
        .unwrap();
    assert!(
        sm.create_comparator()
            .await
            .unwrap()
            .compare_tables(&introspected, &table)
            .unwrap()
            .is_empty()
    );
    let names = sm.list_table_names().await.unwrap();
    assert!(names.contains(&"pgsql_live_schema".to_owned()), "{names:?}");
}
