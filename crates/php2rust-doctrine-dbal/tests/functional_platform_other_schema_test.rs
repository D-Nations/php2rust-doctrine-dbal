//! Port of tests/Functional/Platform/OtherSchemaTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's. Upstream opens the attached file a
//! second time with `DsnParser` and the `sqlite3` driver, which is not
//! ported: here that connection is `pdo_sqlite` with the file as its
//! `path`. The file is in the system's temporary directory (upstream's
//! `/tmp`), so the test also runs on Windows.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/OtherSchemaTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Index, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{DriverManager, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, exec, sqlite};

#[tokio::test]
async fn testATableCanBeCreatedInAnotherSchema() {
    let connection = sqlite();
    let path = std::env::temp_dir().join("test_other_schema.sqlite");
    let path = path.to_str().unwrap();

    exec(
        &connection,
        &format!("ATTACH DATABASE '{}' AS other", path.replace('\'', "''")),
    )
    .await;

    let table = Table::editor()
        .set_unquoted_name("test_other_schema", Some("other"))
        .set_columns(vec![column("id", Types::INTEGER).create().unwrap()])
        .add_index(Index::editor().set_unquoted_column_names(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;
    let mut data = PhpArray::new();
    data.insert("id", Value::Int(1));
    connection
        .insert("other.test_other_schema", &data, &ParameterTypes::new())
        .await
        .unwrap();

    let count = connection
        .fetch_one(
            "SELECT COUNT(*) FROM other.test_other_schema",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
        .unwrap_or(Value::Bool(false));
    assert!(phpunit_equals(&Value::Int(1), &count), "{count:?}");

    let mut params = PhpArray::new();
    params.insert("driver", Value::string("pdo_sqlite"));
    params.insert("path", Value::string(path));
    let connection = DriverManager::get_connection(params, None).unwrap();
    let online_table = connection
        .create_schema_manager()
        .await
        .unwrap()
        .introspect_table_by_unquoted_name("test_other_schema", None)
        .await
        .unwrap();
    assert_eq!(1, online_table.get_indexes().len());
}
