//! Port of tests/Functional/ForeignKeyExceptionTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's. tests/TestUtil.php gives `SQLite`
//! connections the `EnableForeignKeys` middleware; middlewares are not
//! ported, so the test runs the middleware's `PRAGMA foreign_keys=ON`
//! when it connects. A fresh database per test leaves `tearDown()`'s
//! drops nothing to do.
//!
//! Upstream: doctrine/dbal tests/Functional/ForeignKeyExceptionTest.php
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
use php2rust_doctrine_dbal::{Connection, Exception, ParameterTypes};
use schema_manager_support::{
    column, exec, foreign_key, platform, primary_key, schema_manager, sqlite,
};

fn row(pairs: &[(&str, i64)]) -> PhpArray {
    let mut data = PhpArray::new();
    for (name, value) in pairs {
        data.insert(*name, Value::Int(*value));
    }
    data
}

async fn set_up() -> Connection {
    let connection = sqlite();
    exec(&connection, "PRAGMA foreign_keys=ON").await;

    let schema_manager = schema_manager(&connection).await;

    let table = Table::editor()
        .set_unquoted_name("constraint_error_table", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    let owning_table = Table::editor()
        .set_unquoted_name("owning_table", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            column("constraint_id", Types::INTEGER).create().unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .set_foreign_key_constraints(vec![foreign_key(
            None,
            &["constraint_id"],
            "constraint_error_table",
            &["id"],
        )])
        .create()
        .unwrap();

    schema_manager.create_table(&table).await.unwrap();
    schema_manager.create_table(&owning_table).await.unwrap();

    connection
}

async fn insert_rows(connection: &Connection) {
    connection
        .insert(
            "constraint_error_table",
            &row(&[("id", 1)]),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
    connection
        .insert(
            "owning_table",
            &row(&[("id", 1), ("constraint_id", 1)]),
            &ParameterTypes::new(),
        )
        .await
        .unwrap();
}

/// `$this->expectException(ForeignKeyConstraintViolationException::class)`.
fn expect_violation<T: std::fmt::Debug>(result: Result<T, Exception>) {
    let e = result.unwrap_err();
    assert!(
        e.is_instance_of(Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION),
        "{e}"
    );
}

#[tokio::test]
async fn testForeignKeyConstraintViolationExceptionOnInsert() {
    let connection = set_up().await;
    insert_rows(&connection).await;

    expect_violation(
        connection
            .insert(
                "owning_table",
                &row(&[("id", 2), ("constraint_id", 2)]),
                &ParameterTypes::new(),
            )
            .await,
    );
}

#[tokio::test]
async fn testForeignKeyConstraintViolationExceptionOnUpdate() {
    let connection = set_up().await;
    insert_rows(&connection).await;

    expect_violation(
        connection
            .update(
                "constraint_error_table",
                &row(&[("id", 2)]),
                &row(&[("id", 1)]),
                &ParameterTypes::new(),
            )
            .await,
    );
}

#[tokio::test]
async fn testForeignKeyConstraintViolationExceptionOnDelete() {
    let connection = set_up().await;
    insert_rows(&connection).await;

    expect_violation(
        connection
            .delete(
                "constraint_error_table",
                &row(&[("id", 1)]),
                &ParameterTypes::new(),
            )
            .await,
    );
}

#[tokio::test]
async fn testForeignKeyConstraintViolationExceptionOnTruncate() {
    let connection = set_up().await;
    let sql = platform(&connection)
        .await
        .get_truncate_table_sql("constraint_error_table", false);

    insert_rows(&connection).await;

    expect_violation(
        connection
            .execute_statement(&sql, &PhpArray::new(), &ParameterTypes::new())
            .await,
    );
}
