//! Port of tests/Functional/Types/DecimalTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/DecimalTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{ParameterTypes, WrapperParameterType};
use schema_manager_support::{column, drop_and_create_table, platform, sqlite};

/// `stripTrailingZero($expected)`: `rtrim(rtrim($expected, '0'), '.')`.
fn strip_trailing_zero(expected: &str) -> &str {
    expected.trim_end_matches('0').trim_end_matches('.')
}

#[tokio::test]
async fn testInsertAndRetrieveDecimal() {
    // dataValuesProvider()
    for expected in ["13.37", "13.0"] {
        let connection = sqlite();
        let table = Table::editor()
            .set_unquoted_name("decimal_table", None)
            .set_columns(vec![
                column("val", Types::DECIMAL)
                    .set_precision(Some(4))
                    .set_scale(2)
                    .create()
                    .unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let mut data = PhpArray::new();
        data.insert("val", Value::string(expected));
        let types: ParameterTypes =
            [(Key::from("val"), WrapperParameterType::from(Types::DECIMAL))]
                .into_iter()
                .collect();
        connection
            .insert("decimal_table", &data, &types)
            .await
            .unwrap();

        let fetched = connection
            .fetch_one(
                "SELECT val FROM decimal_table",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        let value = types::get_type(Types::DECIMAL)
            .unwrap()
            .convert_to_php_value(&fetched, platform(&connection).await)
            .unwrap();

        let Value::String(value) = value else {
            panic!("{expected}: {value:?}");
        };
        assert_eq!(
            strip_trailing_zero(expected),
            strip_trailing_zero(&value.to_string_lossy()),
        );
    }
}
