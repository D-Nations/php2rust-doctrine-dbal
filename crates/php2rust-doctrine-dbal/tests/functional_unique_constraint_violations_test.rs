//! Port of tests/Functional/UniqueConstraintViolationsTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's. The tests
//! on deferred constraints, which upstream skips on `SQLite`, are not here
//! (parity marks them n/a); so is `testCommitViolatesConstraintWhileUsing
//! TransactionNesting`, which skips on them too. A fresh database per test
//! leaves `tearDown()`'s drop nothing to do.
//!
//! Upstream: doctrine/dbal tests/Functional/UniqueConstraintViolationsTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::PhpArray;
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, Exception, ParameterTypes};
use schema_manager_support::{column, exec, schema_manager, sqlite};

/// `$this->constraintName` on a platform other than Oracle.
const CONSTRAINT_NAME: &str = "c1_unique";

async fn set_up() -> Connection {
    let connection = sqlite();

    let schema_manager = schema_manager(&connection).await;

    let table = Table::editor()
        .set_unquoted_name("unique_constraint_violations", None)
        .set_columns(vec![
            column("unique_field", Types::INTEGER).create().unwrap(),
        ])
        .create()
        .unwrap();

    schema_manager.create_table(&table).await.unwrap();

    // SQLitePlatform
    let create_constraint = format!(
        "CREATE UNIQUE INDEX {CONSTRAINT_NAME} ON unique_constraint_violations(unique_field)"
    );
    exec(&connection, &create_constraint).await;

    exec(
        &connection,
        "INSERT INTO unique_constraint_violations VALUES (1)",
    )
    .await;

    connection
}

async fn insert(connection: &Connection) -> Result<i64, Exception> {
    connection
        .execute_statement(
            "INSERT INTO unique_constraint_violations VALUES (1)",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
}

/// `expectUniqueConstraintViolation(false)`.
fn expect_unique_constraint_violation<T: std::fmt::Debug>(result: Result<T, Exception>) {
    let e = result.unwrap_err();
    assert!(
        e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION),
        "{e}"
    );
}

#[tokio::test]
async fn testTransactionalViolatesConstraint() {
    let connection = set_up().await;

    expect_unique_constraint_violation(
        connection
            .transactional(async |connection: &Connection| insert(connection).await)
            .await,
    );
}

#[tokio::test]
async fn testTransactionalViolatesConstraintWhileUsingTransactionNesting() {
    let connection = set_up().await;
    assert!(
        schema_manager_support::platform(&connection)
            .await
            .supports_savepoints()
    );

    expect_unique_constraint_violation(
        connection
            .transactional(async |connection: &Connection| {
                connection.begin_transaction().await?;

                match insert(connection).await {
                    Ok(n) => Ok(n),
                    Err(t) => {
                        connection.roll_back().await?;
                        Err(t)
                    }
                }
            })
            .await,
    );
}

#[tokio::test]
async fn testInsertViolatesConstraint() {
    let connection = set_up().await;

    connection.begin_transaction().await.unwrap();

    let result = match insert(&connection).await {
        Ok(n) => Ok(n),
        Err(t) => {
            connection.roll_back().await.unwrap();
            Err(t)
        }
    };
    expect_unique_constraint_violation(result);
}
