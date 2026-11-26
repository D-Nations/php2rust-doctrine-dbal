//! Port of tests/Functional/Schema/DefaultValueTest.php on `pdo_sqlite` in
//! memory. `setUp()` runs once per test, whose data provider's cases run
//! in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/DefaultValueTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

/// `columnProvider()`: potential escaped literals from all platforms
/// combined.
const COLUMNS: [(&str, &str, Option<&str>); 17] = [
    ("Single quote", "single_quote", Some("foo'bar")),
    (
        "Single quote, doubled",
        "single_quote_doubled",
        Some("foo''bar"),
    ),
    ("Double quote", "double_quote", Some("foo\"bar")),
    (
        "Double quote, doubled",
        "double_quote_doubled",
        Some("foo\"\"bar"),
    ),
    ("Backspace", "backspace", Some("foo\x08bar")),
    ("New line", "new_line", Some("foo\nbar")),
    ("Carriage return", "carriage_return", Some("foo\rbar")),
    ("Tab", "tab", Some("foo\tbar")),
    ("Substitute", "substitute", Some("foo\x1abar")),
    ("Backslash", "backslash", Some("foo\\bar")),
    (
        "Backslash, doubled",
        "backslash_doubled",
        Some("foo\\\\bar"),
    ),
    ("Percent", "percent_sign", Some("foo%bar")),
    ("Underscore", "underscore", Some("foo_bar")),
    ("NULL string", "null_string", Some("NULL")),
    ("NULL value", "null_value", None),
    (
        "SQL expression",
        "sql_expression",
        Some("'; DROP DATABASE doctrine --"),
    ),
    ("No double conversion", "no_double_conversion", Some("\\'")),
];

fn expected(default: Option<&str>) -> Value {
    default.map_or(Value::Null, Value::string)
}

/// `setUp()`.
async fn set_up() -> Connection {
    let connection = sqlite();
    let mut editor = Table::editor()
        .set_unquoted_name("default_value", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()]);

    for (_, name, default) in COLUMNS {
        editor = editor.add_column(
            column(name, Types::STRING)
                .set_length(Some(32))
                .set_default_value(expected(default))
                .set_not_null(false)
                .create()
                .unwrap(),
        );
    }

    let table = editor.create().unwrap();
    drop_and_create_table(&connection, &table).await;

    let mut data = PhpArray::new();
    data.insert("id", Value::Int(1));
    connection
        .insert("default_value", &data, &ParameterTypes::new())
        .await
        .unwrap();
    connection
}

#[tokio::test]
async fn testEscapedDefaultValueCanBeIntrospected() {
    let connection = set_up().await;
    let table = schema_manager(&connection)
        .await
        .introspect_table_by_unquoted_name("default_value", None)
        .await
        .unwrap();

    for (case, name, default) in COLUMNS {
        assert!(
            expected(default).identical(table.get_column(name).unwrap().get_default()),
            "{case}"
        );
    }
}

#[tokio::test]
async fn testEscapedDefaultValueCanBeInserted() {
    let connection = set_up().await;

    for (case, name, default) in COLUMNS {
        let value = connection
            .fetch_one(
                &format!("SELECT {name} FROM default_value"),
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(expected(default).identical(&value), "{case}: {value:?}");
    }
}
