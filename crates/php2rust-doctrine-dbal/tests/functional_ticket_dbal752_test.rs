//! Port of tests/Functional/Ticket/DBAL752Test.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory
//! (the only platform upstream runs it on).
//!
//! Upstream: doctrine/dbal tests/Functional/Ticket/DBAL752Test.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php2rust_doctrine_dbal::schema::AbstractSchemaManager;
use schema_manager_support::{exec, schema_manager, sqlite};

const SMALL_INT: &str = "Doctrine\\DBAL\\Types\\SmallIntType";
const INTEGER: &str = "Doctrine\\DBAL\\Types\\IntegerType";
const BIG_INT: &str = "Doctrine\\DBAL\\Types\\BigIntType";

#[tokio::test]
async fn testUnsignedIntegerDetection() {
    let connection = sqlite();
    exec(
        &connection,
        r#"CREATE TABLE dbal752_unsigneds (
    small SMALLINT,
    small_unsigned SMALLINT UNSIGNED,
    medium MEDIUMINT,
    medium_unsigned MEDIUMINT UNSIGNED,
    "integer" INTEGER,
    integer_unsigned INTEGER UNSIGNED,
    big BIGINT,
    big_unsigned BIGINT UNSIGNED
);"#,
    )
    .await;

    let schema_manager = schema_manager(&connection).await;

    let fetched_table = schema_manager
        .introspect_table_by_unquoted_name("dbal752_unsigneds", None)
        .await
        .unwrap();

    let class = |name: &str| {
        fetched_table
            .get_column(name)
            .unwrap()
            .get_type()
            .unwrap()
            .class()
            .to_owned()
    };
    let unsigned = |name: &str| fetched_table.get_column(name).unwrap().get_unsigned();

    assert_eq!(SMALL_INT, class("small"));
    assert_eq!(SMALL_INT, class("small_unsigned"));
    assert_eq!(INTEGER, class("medium"));
    assert_eq!(INTEGER, class("medium_unsigned"));
    assert_eq!(INTEGER, class("integer"));
    assert_eq!(INTEGER, class("integer_unsigned"));
    assert_eq!(BIG_INT, class("big"));
    assert_eq!(BIG_INT, class("big_unsigned"));

    assert!(unsigned("small_unsigned"));
    assert!(unsigned("medium_unsigned"));
    assert!(unsigned("integer_unsigned"));
    assert!(unsigned("big_unsigned"));

    assert!(!unsigned("small"));
    assert!(!unsigned("medium"));
    assert!(!unsigned("integer"));
    assert!(!unsigned("big"));
}
