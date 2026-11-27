//! Port of tests/Functional/Platform/AlterDecimalColumnTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's; a data
//! provider's cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/AlterDecimalColumnTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use schema_manager_support::{column, drop_and_create_table, schema_manager, sqlite};

/// `scaleAndPrecisionProvider()`: new precision, new scale and type, for
/// `Types::DECIMAL`, then `Types::NUMBER`.
fn scale_and_precision_provider() -> Vec<(String, i64, i64, &'static str)> {
    let mut cases = Vec::new();
    for type_name in [Types::DECIMAL, Types::NUMBER] {
        cases.push((format!("Precision ({type_name})"), 12, 6, type_name));
        cases.push((format!("Scale ({type_name})"), 16, 8, type_name));
        cases.push((
            format!("Precision and scale ({type_name})"),
            10,
            4,
            type_name,
        ));
    }
    cases
}

#[tokio::test]
async fn testAlterPrecisionAndScale() {
    for (case, new_precision, new_scale, type_name) in scale_and_precision_provider() {
        let connection = sqlite();
        let table = Table::editor()
            .set_unquoted_name("decimal_table", None)
            .set_columns(vec![
                column("val", type_name)
                    .set_precision(Some(16))
                    .set_scale(6)
                    .create()
                    .unwrap(),
            ])
            .create()
            .unwrap();

        drop_and_create_table(&connection, &table).await;

        let table = table
            .edit()
            .unwrap()
            .modify_column_by_unquoted_name("val", |editor| {
                editor
                    .set_precision(Some(new_precision))
                    .set_scale(new_scale)
            })
            .create()
            .unwrap();

        let schema_manager = schema_manager(&connection).await;

        let diff = schema_manager
            .create_comparator()
            .compare_tables(
                &schema_manager
                    .introspect_table_by_unquoted_name("decimal_table", None)
                    .await
                    .unwrap(),
                &table,
            )
            .unwrap();

        schema_manager.alter_table(&diff).await.unwrap();

        let table = schema_manager
            .introspect_table_by_unquoted_name("decimal_table", None)
            .await
            .unwrap();
        let column = table.get_column("val").unwrap();

        assert_eq!(Some(new_precision), column.get_precision(), "{case}");
        assert_eq!(new_scale, column.get_scale(), "{case}");
    }
}
