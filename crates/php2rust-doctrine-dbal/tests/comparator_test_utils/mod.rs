//! Port of tests/Functional/Schema/ComparatorTestUtils.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879: the
//! comparator functional tests' helpers.
//!
//! Upstream: doctrine/dbal tests/Functional/Schema/ComparatorTestUtils.php
#![allow(dead_code)]

use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Comparator, SQLiteSchemaManager, Table, TableDiff,
};
use php2rust_doctrine_dbal::{Connection, Exception};

use crate::schema_manager_support::schema_manager;

async fn introspect(
    schema_manager: &SQLiteSchemaManager<'_>,
    desired_table: &Table,
) -> Result<Table, Exception> {
    schema_manager
        .introspect_table(&desired_table.get_object_name()?.to_string())
        .await
}

/// `diffFromActualToDesiredTable($schemaManager, $comparator,
/// $desiredTable)`.
pub async fn diff_from_actual_to_desired_table(
    schema_manager: &SQLiteSchemaManager<'_>,
    comparator: &Comparator<'_>,
    desired_table: &Table,
) -> Result<TableDiff, Exception> {
    comparator.compare_tables(
        &introspect(schema_manager, desired_table).await?,
        desired_table,
    )
}

/// `diffFromDesiredToActualTable($schemaManager, $comparator,
/// $desiredTable)`.
pub async fn diff_from_desired_to_actual_table(
    schema_manager: &SQLiteSchemaManager<'_>,
    comparator: &Comparator<'_>,
    desired_table: &Table,
) -> Result<TableDiff, Exception> {
    comparator.compare_tables(
        desired_table,
        &introspect(schema_manager, desired_table).await?,
    )
}

/// `assertDiffNotEmpty($connection, $comparator, $table)`.
pub async fn assert_diff_not_empty(
    connection: &Connection,
    comparator: &Comparator<'_>,
    table: &Table,
) {
    let schema_manager = schema_manager(connection).await;

    let diff = diff_from_actual_to_desired_table(&schema_manager, comparator, table)
        .await
        .unwrap();

    assert!(!diff.is_empty());

    schema_manager.alter_table(&diff).await.unwrap();

    assert!(
        diff_from_actual_to_desired_table(&schema_manager, comparator, table)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        diff_from_desired_to_actual_table(&schema_manager, comparator, table)
            .await
            .unwrap()
            .is_empty()
    );
}
