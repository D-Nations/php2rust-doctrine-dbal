//! Port of tests/Schema/Introspection/IntrospectingSchemaProviderTest.php.
//! PHPUnit's stub of `MetadataProvider` is a provider that returns no
//! rows and does not implement the optional unique constraint methods.
//!
//! Upstream: doctrine/dbal tests/Schema/Introspection/IntrospectingSchemaProviderTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::deprecations;
use php2rust_doctrine_dbal::schema::SchemaConfig;
use php2rust_doctrine_dbal::schema::introspection::IntrospectingSchemaProvider;
use php2rust_doctrine_dbal::schema::metadata::{
    DatabaseMetadataRow, ForeignKeyConstraintColumnMetadataRow, IndexColumnMetadataRow,
    MetadataProvider, PrimaryKeyConstraintColumnRow, SchemaMetadataRow, SequenceMetadataRow,
    TableColumnMetadataRow, TableMetadataRow, ViewMetadataRow,
};

const LINK_7461: &str = "https://github.com/doctrine/dbal/pull/7461";

type Rows<T> = Result<Vec<T>, Exception>;

/// `self::createStub(MetadataProvider::class)`.
struct Stub;

impl MetadataProvider for Stub {
    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_all_database_names(&self) -> Rows<DatabaseMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_all_schema_names(&self) -> Rows<SchemaMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_all_table_names(&self) -> Rows<TableMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_table_columns_for_all_tables(&self) -> Rows<TableColumnMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_table_columns_for_table(
        &self,
        _schema_name: Option<&str>,
        _table_name: &str,
    ) -> Rows<TableColumnMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_index_columns_for_all_tables(&self) -> Rows<IndexColumnMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_index_columns_for_table(
        &self,
        _schema_name: Option<&str>,
        _table_name: &str,
    ) -> Rows<IndexColumnMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_primary_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_primary_key_constraint_columns_for_table(
        &self,
        _schema_name: Option<&str>,
        _table_name: &str,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_foreign_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_foreign_key_constraint_columns_for_table(
        &self,
        _schema_name: Option<&str>,
        _table_name: &str,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_table_options_for_all_tables(&self) -> Rows<TableMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_table_options_for_table(
        &self,
        _schema_name: Option<&str>,
        _table_name: &str,
    ) -> Rows<TableMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_all_views(&self) -> Rows<ViewMetadataRow> {
        Ok(Vec::new())
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_all_sequences(&self) -> Rows<SequenceMetadataRow> {
        Ok(Vec::new())
    }

    fn class(&self) -> &'static str {
        "MockObject_MetadataProvider"
    }
}

/// `createProvider()`.
fn create_provider() -> IntrospectingSchemaProvider<Stub> {
    IntrospectingSchemaProvider::new(Stub, None, SchemaConfig::new().to_table_configuration())
}

#[tokio::test]
async fn testGetUniqueConstraintsForTableWithoutMetadataSupport() {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(LINK_7461);

    assert_eq!(
        create_provider()
            .get_unique_constraints_for_table(None, "orders")
            .await
            .unwrap(),
        [] as [php2rust_doctrine_dbal::schema::UniqueConstraint; 0]
    );

    assert!(deprecations::count(LINK_7461) > before);
    deprecations::disable();
}

#[tokio::test]
async fn testGetAllTablesWithoutUniqueConstraintMetadataSupport() {
    deprecations::enable_tracking_deprecations();
    let before = deprecations::count(LINK_7461);

    assert!(create_provider().get_all_tables().await.unwrap().is_empty());

    assert!(deprecations::count(LINK_7461) > before);
    deprecations::disable();
}
