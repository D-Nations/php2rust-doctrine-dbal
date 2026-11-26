//! Port of tests/Schema/Metadata/ForeignKeyConstraintColumnMetadataRowTest.php.
//!
//! Upstream: doctrine/dbal tests/Schema/Metadata/ForeignKeyConstraintColumnMetadataRowTest.php
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::schema::metadata::{
    ForeignKeyConstraintColumnMetadataRow, ForeignKeyConstraintColumnMetadataRowParts,
};
use php2rust_doctrine_dbal::schema::{MatchType, ReferentialAction};

#[test]
fn testNeitherIdNorName() {
    let result =
        ForeignKeyConstraintColumnMetadataRow::new(ForeignKeyConstraintColumnMetadataRowParts {
            referencing_schema_name: None,
            referencing_table_name: "orders",
            id: None,
            name: None,
            referenced_schema_name: None,
            referenced_table_name: "customers",
            match_type: MatchType::Simple,
            on_update_action: ReferentialAction::NoAction,
            on_delete_action: ReferentialAction::NoAction,
            is_deferrable: false,
            is_deferred: false,
            referencing_column_name: "customer_id",
            referenced_column_name: "id",
        });

    let Err(e) = result else {
        panic!("expected InvalidArgumentException");
    };
    assert!(
        e.is_instance_of(Exception::INVALID_ARGUMENT_EXCEPTION),
        "{e}"
    );
}
