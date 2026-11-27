//! Port of tests/Functional/Schema/CustomIntrospectionTest.php on
//! `pdo_sqlite` in memory, with tests/Functional/Schema/Types/MoneyType.php.
//! Only the SQL declaration of `MoneyType` takes part in the test; its
//! `Money` value conversions are not ported.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/CustomIntrospectionTest.php
//! Upstream: doctrine/dbal tests/Functional/Schema/Types/MoneyType.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{PhpArray, PhpString};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::platforms::AbstractPlatform;
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::{self, Type, TypeArgument, Types};
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};
use std::sync::Arc;

/// `MoneyType`.
#[derive(Debug)]
struct MoneyType;

impl MoneyType {
    const NAME: &'static str = "money";
}

impl Type for MoneyType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Tests\\Functional\\Schema\\Types\\MoneyType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_decimal_type_declaration_sql(column)
    }
}

/// `setUpBeforeClass()`.
fn set_up_before_class() {
    if !types::has_type(MoneyType::NAME) {
        types::add_type(MoneyType::NAME, TypeArgument::Instance(Arc::new(MoneyType))).unwrap();
    }
}

#[tokio::test]
async fn testCustomColumnIntrospection() {
    set_up_before_class();
    let connection = sqlite();

    let table = Table::editor()
        .set_unquoted_name("test_custom_column_introspection", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("quantity", Types::DECIMAL)
                .set_precision(Some(10))
                .set_scale(2)
                .set_not_null(false)
                .create()
                .unwrap(),
            column("amount", "money")
                .set_precision(Some(10))
                .set_scale(2)
                .set_not_null(false)
                .create()
                .unwrap(),
        ])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let schema_manager = schema_manager(&connection).await;
    let online_table = schema_manager
        .introspect_table_by_unquoted_name("test_custom_column_introspection", None)
        .await
        .unwrap();
    let diff = schema_manager
        .create_comparator()
        .await
        .unwrap()
        .compare_tables(&online_table, &table)
        .unwrap();
    assert!(
        diff.is_empty(),
        "Tables should be identical. Differences detected in {}.",
        diff.get_changed_columns()
            .iter()
            .map(|column_diff| column_diff
                .get_old_column()
                .get_object_name()
                .unwrap()
                .to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
}
