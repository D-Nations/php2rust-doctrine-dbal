//! Port of tests/Functional/Types/GuidTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//!
//! Upstream: doctrine/dbal tests/Functional/Types/GuidTest.php
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
use schema_manager_support::{column, drop_and_create_table, sqlite};

#[tokio::test]
async fn testInsertAndSelect() {
    let connection = sqlite();
    let table = Table::editor()
        .set_unquoted_name("guid_table", None)
        .set_columns(vec![column("guid", Types::GUID).create().unwrap()])
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    let guid = "7c620eda-ea79-11eb-9a03-0242ac130003";

    let mut data = PhpArray::new();
    data.insert("guid", Value::string(guid));
    let result = connection
        .insert("guid_table", &data, &ParameterTypes::new())
        .await
        .unwrap();
    assert_eq!(1, result);

    let value = connection
        .fetch_one(
            "SELECT guid FROM guid_table",
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .unwrap()
        .unwrap();

    // the platforms with native UUID support inconsistently format the binary value
    // as a string using the lower or the upper case; this is acceptable since
    // regardless of the case they encode the same binary value
    let Value::String(value) = value else {
        panic!("{value:?}");
    };
    assert_eq!(guid.to_lowercase(), value.to_string_lossy().to_lowercase());
}
