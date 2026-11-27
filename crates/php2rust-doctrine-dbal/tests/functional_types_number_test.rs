//! Port of tests/Functional/Types/NumberTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! `#[RequiresPhp('>= 8.4')]` and `#[RequiresPhpExtension('bcmath')]`:
//! the oracle's PHP 8.4.27 runs it.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/NumberTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use std::sync::Arc;

use php_value::bcmath::BcNumber;
use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{ParameterTypes, WrapperParameterType};
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

fn number_table() -> Table {
    Table::editor()
        .set_unquoted_name("number_table", None)
        .set_columns(vec![
            column("val", Types::NUMBER)
                .set_precision(Some(4))
                .set_scale(2)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap()
}

/// `#[TestWith(['13.37'])]`, `#[TestWith(['13.0'])]`.
#[tokio::test]
async fn testInsertAndRetrieveNumber() {
    for number_as_string in ["13.37", "13.0"] {
        let connection = sqlite();
        let expected = BcNumber::parse(number_as_string.as_bytes()).unwrap();

        drop_and_create_table(&connection, &number_table()).await;

        let mut data = PhpArray::new();
        data.insert("val", Value::Object(Arc::new(expected.clone())));
        let types: ParameterTypes = [(Key::from("val"), WrapperParameterType::from(Types::NUMBER))]
            .into_iter()
            .collect();
        connection
            .insert("number_table", &data, &types)
            .await
            .unwrap();

        let fetched = connection
            .fetch_one(
                "SELECT val FROM number_table",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        let value = connection
            .convert_to_php_value(&fetched, Types::NUMBER)
            .await
            .unwrap();

        let value = value
            .as_object()
            .and_then(|object| object.downcast_ref::<BcNumber>())
            .unwrap_or_else(|| panic!("{number_as_string}: not a BcMath\\Number"));
        assert!(expected.compare(value).is_eq(), "{number_as_string}");
    }
}

#[tokio::test]
async fn testCompareNumberTable() {
    let connection = sqlite();
    let table = number_table();

    drop_and_create_table(&connection, &table).await;

    let schema_manager = schema_manager(&connection).await;

    assert!(
        schema_manager
            .create_comparator()
            .compare_tables(
                &schema_manager
                    .introspect_table_by_unquoted_name("number_table", None)
                    .await
                    .unwrap(),
                &table,
            )
            .unwrap()
            .is_empty()
    );
}
