//! Port of tests/Functional/Types/BigIntTypeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory
//! (64-bit `PHP_INT_SIZE`). `testUnsignedBigIntOnMySQL`, which upstream
//! skips on `SQLite`, is not here (parity marks it n/a).
//!
//! Upstream: doctrine/dbal tests/Functional/Types/BigIntTypeTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, exec, primary_key, sqlite};

/// `provideBigIntLiterals()`.
fn provide_big_int_literals() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("zero", "0", Value::Int(0)),
        ("null", "null", Value::Null),
        ("positive number", "42", Value::Int(42)),
        ("negative number", "-42", Value::Int(-42)),
        (
            "large positive number",
            "9223372036854775806",
            Value::Int(i64::MAX - 1),
        ),
        (
            "large negative number",
            "-9223372036854775807",
            Value::Int(i64::MIN + 1),
        ),
        (
            "largest positive number",
            "9223372036854775807",
            Value::Int(i64::MAX),
        ),
        (
            "largest negative number",
            "-9223372036854775808",
            Value::Int(i64::MIN),
        ),
    ]
}

#[tokio::test]
async fn testSelectBigInt() {
    for (name, sql_literal, expected_value) in provide_big_int_literals() {
        let connection = sqlite();
        let table = Table::editor()
            .set_unquoted_name("bigint_type_test", None)
            .set_columns(vec![
                column("id", Types::SMALLINT).create().unwrap(),
                column("my_integer", Types::BIGINT)
                    .set_not_null(false)
                    .create()
                    .unwrap(),
            ])
            .set_primary_key_constraint(primary_key(&["id"]))
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        exec(
            &connection,
            &format!("INSERT INTO bigint_type_test (id, my_integer)\nVALUES (42, {sql_literal})"),
        )
        .await;

        let fetched = connection
            .fetch_one(
                "SELECT my_integer from bigint_type_test WHERE id = 42",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        let actual = connection
            .convert_to_php_value(&fetched, Types::BIGINT)
            .await
            .unwrap();

        assert!(
            expected_value.identical(&actual),
            "{name}: expected {expected_value:?}, got {actual:?}"
        );
    }
}
