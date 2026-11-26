//! Port of tests/Schema/Metadata/UniqueConstraintColumnMetadataRowTest.php.
//!
//! Upstream: doctrine/dbal tests/Schema/Metadata/UniqueConstraintColumnMetadataRowTest.php
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::schema::metadata::UniqueConstraintColumnMetadataRow;

#[test]
fn testNeitherIdNorName() {
    let result = UniqueConstraintColumnMetadataRow::new(None, "orders", None, None, "user_id");

    let Err(e) = result else {
        panic!("expected InvalidArgumentException");
    };
    assert!(
        e.is_instance_of(Exception::INVALID_ARGUMENT_EXCEPTION),
        "{e}"
    );
}
