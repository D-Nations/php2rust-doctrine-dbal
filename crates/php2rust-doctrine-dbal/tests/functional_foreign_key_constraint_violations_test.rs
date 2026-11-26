//! Port of tests/Functional/ForeignKeyConstraintViolationsTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879, on
//! `pdo_sqlite` in memory. Names and assertions are PHPUnit's. The tests
//! on deferred constraints, which upstream skips on `SQLite`, are not here
//! (parity marks them n/a). tests/TestUtil.php gives `SQLite` connections
//! the `EnableForeignKeys` middleware; middlewares are not ported, so the
//! test runs the middleware's `PRAGMA foreign_keys=ON` when it connects.
//! A fresh database per test leaves `tearDown()`'s drops nothing to do.
//!
//! Upstream: doctrine/dbal tests/Functional/ForeignKeyConstraintViolationsTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::PhpArray;
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, ForeignKeyConstraint, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, Exception, ParameterTypes};
use schema_manager_support::{column, exec, primary_key, schema_manager, sqlite};

const CONSTRAINT_NAME: &str = "fk1";

async fn set_up() -> Connection {
    let connection = sqlite();
    exec(&connection, "PRAGMA foreign_keys=ON").await;

    let schema_manager = schema_manager(&connection).await;

    let table = Table::editor()
        .set_unquoted_name("test_t1", None)
        .set_columns(vec![column("ref_id", Types::INTEGER).create().unwrap()])
        .create()
        .unwrap();

    schema_manager.create_table(&table).await.unwrap();

    let table2 = Table::editor()
        .set_unquoted_name("test_t2", None)
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    schema_manager.create_table(&table2).await.unwrap();

    let create_constraint = ForeignKeyConstraint::new(
        ["ref_id"],
        "test_t2",
        ["id"],
        CONSTRAINT_NAME,
        PhpArray::new(),
    )
    .unwrap();

    schema_manager
        .create_foreign_key(&create_constraint, "test_t1")
        .await
        .unwrap();

    connection
}

async fn insert(connection: &Connection) -> Result<i64, Exception> {
    connection
        .execute_statement(
            "INSERT INTO test_t1 VALUES (1)",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
}

/// `expectConstraintViolation(false)`: a non-deferred violation.
fn expect_constraint_violation<T: std::fmt::Debug>(result: Result<T, Exception>) {
    let e = result.unwrap_err();
    assert!(
        e.is_instance_of(Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION),
        "{e}"
    );
}

#[tokio::test]
async fn testTransactionalViolatesConstraint() {
    let connection = set_up().await;

    expect_constraint_violation(
        connection
            .transactional(async |connection: &Connection| insert(connection).await)
            .await,
    );
}

#[tokio::test]
async fn testTransactionalViolatesConstraintWhileUsingTransactionNesting() {
    let connection = set_up().await;
    assert!(platform_supports_savepoints(&connection).await);

    expect_constraint_violation(
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
    expect_constraint_violation(result);
}

async fn platform_supports_savepoints(connection: &Connection) -> bool {
    schema_manager_support::platform(connection)
        .await
        .supports_savepoints()
}
