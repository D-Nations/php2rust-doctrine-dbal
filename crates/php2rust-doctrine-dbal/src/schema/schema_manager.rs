//! The schema manager a connection hands out: `AbstractPlatform::createSchemaManager()`
//! returns an `AbstractSchemaManager` of the platform's class, and the
//! trait is not object-safe (its methods are `async`), so the managers
//! ported are the variants of [`SchemaManager`], which forwards each
//! method of the trait to its variant. Its metadata provider is
//! [`SchemaMetadataProvider`] and its comparator [`SchemaComparator`], in
//! the same way.
//!
//! Upstream: none

use std::ops::Deref;

use php_value::PhpArray;

use crate::Connection;
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;
use crate::platforms::mysql::{Comparator as MySQLComparator, MySQLMetadataProvider};
use crate::platforms::sqlite::{Comparator as SQLiteComparator, SQLiteMetadataProvider};

use super::abstract_schema_manager::{AbstractSchemaManager, Row};
use super::array_map::ArrayMap;
use super::column::Column;
use super::comparator::Comparator;
use super::comparator_config::ComparatorConfig;
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::Index;
use super::metadata::{
    DatabaseMetadataRow, ForeignKeyConstraintColumnMetadataRow, IndexColumnMetadataRow,
    MetadataProvider, PrimaryKeyConstraintColumnRow, SchemaMetadataRow, SequenceMetadataRow,
    TableColumnMetadataRow, TableMetadataRow, UniqueConstraintColumnMetadataRow, ViewMetadataRow,
};
use super::mysql_schema_manager::MySQLSchemaManager;
use super::name::{OptionallyQualifiedName, UnqualifiedName};
use super::primary_key_constraint::PrimaryKeyConstraint;
use super::schema::Schema;
use super::schema_config::SchemaConfig;
use super::schema_diff::SchemaDiff;
use super::sequence::Sequence;
use super::sqlite_schema_manager::SQLiteSchemaManager;
use super::table::Table;
use super::table_diff::TableDiff;
use super::unique_constraint::UniqueConstraint;
use super::view::View;

type R<T> = Result<T, Exception>;
type Rows<T> = Result<Vec<T>, Exception>;

/// The schema manager of a connection's platform.
#[derive(Debug)]
pub enum SchemaManager<'c> {
    /// `SQLiteSchemaManager`.
    SQLite(SQLiteSchemaManager<'c>),
    /// `MySQLSchemaManager`, for the `MySQL` and `MariaDB` platforms.
    MySQL(MySQLSchemaManager<'c>),
}

impl<'c> SchemaManager<'c> {
    /// `createComparator()`.
    pub async fn create_comparator(&self) -> R<SchemaComparator<'c>> {
        self.create_comparator_with(ComparatorConfig::default())
            .await
    }

    /// `createComparator($config)`: the platform's comparator. `MySQL`'s
    /// reads the database's default character set and collation.
    pub async fn create_comparator_with(
        &self,
        config: ComparatorConfig,
    ) -> R<SchemaComparator<'c>> {
        Ok(match self {
            Self::SQLite(m) => SchemaComparator::SQLite(m.create_comparator_with(config)),
            Self::MySQL(m) => SchemaComparator::MySQL(m.create_comparator_with(config).await?),
        })
    }

    /// `migrateSchema($newSchema)`.
    pub async fn migrate_schema(&self, new_schema: &Schema) -> R<()> {
        match self {
            Self::SQLite(m) => m.migrate_schema(new_schema).await,
            Self::MySQL(m) => m.migrate_schema(new_schema).await,
        }
    }

    /// The `SQLite` manager, if it is one.
    pub fn as_sqlite(&self) -> Option<&SQLiteSchemaManager<'c>> {
        match self {
            Self::SQLite(m) => Some(m),
            Self::MySQL(_) => None,
        }
    }

    /// The `MySQL` manager, if it is one.
    pub fn as_mysql(&self) -> Option<&MySQLSchemaManager<'c>> {
        match self {
            Self::MySQL(m) => Some(m),
            Self::SQLite(_) => None,
        }
    }
}

impl<'c> From<SQLiteSchemaManager<'c>> for SchemaManager<'c> {
    fn from(manager: SQLiteSchemaManager<'c>) -> Self {
        Self::SQLite(manager)
    }
}

impl<'c> From<MySQLSchemaManager<'c>> for SchemaManager<'c> {
    fn from(manager: MySQLSchemaManager<'c>) -> Self {
        Self::MySQL(manager)
    }
}

/// The comparator of a [`SchemaManager`]: it dereferences to the
/// `Comparator` it customizes.
#[derive(Debug)]
pub enum SchemaComparator<'c> {
    SQLite(SQLiteComparator<'c>),
    MySQL(MySQLComparator<'c>),
}

impl<'c> Deref for SchemaComparator<'c> {
    type Target = Comparator<'c>;

    fn deref(&self) -> &Comparator<'c> {
        match self {
            Self::SQLite(c) => c,
            Self::MySQL(c) => c,
        }
    }
}

/// The metadata provider of a [`SchemaManager`].
#[derive(Debug)]
pub enum SchemaMetadataProvider<'c> {
    SQLite(SQLiteMetadataProvider<'c>),
    MySQL(MySQLMetadataProvider<'c>),
}

impl MetadataProvider for SchemaMetadataProvider<'_> {
    async fn get_all_database_names(&self) -> Rows<DatabaseMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_all_database_names().await,
            Self::MySQL(m) => m.get_all_database_names().await,
        }
    }

    async fn get_all_schema_names(&self) -> Rows<SchemaMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_all_schema_names().await,
            Self::MySQL(m) => m.get_all_schema_names().await,
        }
    }

    async fn get_all_table_names(&self) -> Rows<TableMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_all_table_names().await,
            Self::MySQL(m) => m.get_all_table_names().await,
        }
    }

    async fn get_table_columns_for_all_tables(&self) -> Rows<TableColumnMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_table_columns_for_all_tables().await,
            Self::MySQL(m) => m.get_table_columns_for_all_tables().await,
        }
    }

    async fn get_table_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableColumnMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_table_columns_for_table(schema_name, table_name).await,
            Self::MySQL(m) => m.get_table_columns_for_table(schema_name, table_name).await,
        }
    }

    async fn get_index_columns_for_all_tables(&self) -> Rows<IndexColumnMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_index_columns_for_all_tables().await,
            Self::MySQL(m) => m.get_index_columns_for_all_tables().await,
        }
    }

    async fn get_index_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<IndexColumnMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_index_columns_for_table(schema_name, table_name).await,
            Self::MySQL(m) => m.get_index_columns_for_table(schema_name, table_name).await,
        }
    }

    async fn get_primary_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        match self {
            Self::SQLite(m) => m.get_primary_key_constraint_columns_for_all_tables().await,
            Self::MySQL(m) => m.get_primary_key_constraint_columns_for_all_tables().await,
        }
    }

    async fn get_primary_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        match self {
            Self::SQLite(m) => {
                m.get_primary_key_constraint_columns_for_table(schema_name, table_name)
                    .await
            }
            Self::MySQL(m) => {
                m.get_primary_key_constraint_columns_for_table(schema_name, table_name)
                    .await
            }
        }
    }

    async fn get_unique_constraint_columns_for_all_tables(
        &self,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        match self {
            Self::SQLite(m) => m.get_unique_constraint_columns_for_all_tables().await,
            Self::MySQL(m) => m.get_unique_constraint_columns_for_all_tables().await,
        }
    }

    async fn get_unique_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        match self {
            Self::SQLite(m) => {
                m.get_unique_constraint_columns_for_table(schema_name, table_name)
                    .await
            }
            Self::MySQL(m) => {
                m.get_unique_constraint_columns_for_table(schema_name, table_name)
                    .await
            }
        }
    }

    async fn get_foreign_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_foreign_key_constraint_columns_for_all_tables().await,
            Self::MySQL(m) => m.get_foreign_key_constraint_columns_for_all_tables().await,
        }
    }

    async fn get_foreign_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        match self {
            Self::SQLite(m) => {
                m.get_foreign_key_constraint_columns_for_table(schema_name, table_name)
                    .await
            }
            Self::MySQL(m) => {
                m.get_foreign_key_constraint_columns_for_table(schema_name, table_name)
                    .await
            }
        }
    }

    async fn get_table_options_for_all_tables(&self) -> Rows<TableMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_table_options_for_all_tables().await,
            Self::MySQL(m) => m.get_table_options_for_all_tables().await,
        }
    }

    async fn get_table_options_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_table_options_for_table(schema_name, table_name).await,
            Self::MySQL(m) => m.get_table_options_for_table(schema_name, table_name).await,
        }
    }

    async fn get_all_views(&self) -> Rows<ViewMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_all_views().await,
            Self::MySQL(m) => m.get_all_views().await,
        }
    }

    async fn get_all_sequences(&self) -> Rows<SequenceMetadataRow> {
        match self {
            Self::SQLite(m) => m.get_all_sequences().await,
            Self::MySQL(m) => m.get_all_sequences().await,
        }
    }

    fn class(&self) -> &str {
        match self {
            Self::SQLite(m) => m.class(),
            Self::MySQL(m) => m.class(),
        }
    }
}

impl<'c> AbstractSchemaManager<'c> for SchemaManager<'c> {
    type MetadataProvider = SchemaMetadataProvider<'c>;

    fn connection(&self) -> &'c Connection {
        match self {
            Self::SQLite(m) => m.connection(),
            Self::MySQL(m) => m.connection(),
        }
    }

    fn platform(&self) -> &'c dyn AbstractPlatform {
        match self {
            Self::SQLite(m) => m.platform(),
            Self::MySQL(m) => m.platform(),
        }
    }

    async fn create_metadata_provider(&self) -> R<SchemaMetadataProvider<'c>> {
        Ok(match self {
            Self::SQLite(m) => SchemaMetadataProvider::SQLite(m.create_metadata_provider().await?),
            Self::MySQL(m) => SchemaMetadataProvider::MySQL(m.create_metadata_provider().await?),
        })
    }

    async fn select_table_names(&self, database_name: &str) -> R<crate::Result<'c>> {
        match self {
            Self::SQLite(m) => m.select_table_names(database_name).await,
            Self::MySQL(m) => m.select_table_names(database_name).await,
        }
    }

    async fn select_table_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        match self {
            Self::SQLite(m) => m.select_table_columns(database_name, table_name).await,
            Self::MySQL(m) => m.select_table_columns(database_name, table_name).await,
        }
    }

    async fn select_index_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        match self {
            Self::SQLite(m) => m.select_index_columns(database_name, table_name).await,
            Self::MySQL(m) => m.select_index_columns(database_name, table_name).await,
        }
    }

    async fn select_foreign_key_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        match self {
            Self::SQLite(m) => {
                m.select_foreign_key_columns(database_name, table_name)
                    .await
            }
            Self::MySQL(m) => {
                m.select_foreign_key_columns(database_name, table_name)
                    .await
            }
        }
    }

    async fn fetch_table_options_by_table(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<ArrayMap<PhpArray>> {
        match self {
            Self::SQLite(m) => {
                m.fetch_table_options_by_table(database_name, table_name)
                    .await
            }
            Self::MySQL(m) => {
                m.fetch_table_options_by_table(database_name, table_name)
                    .await
            }
        }
    }

    fn get_portable_table_column_definition(&self, table_column: &Row) -> R<Column> {
        match self {
            Self::SQLite(m) => m.get_portable_table_column_definition(table_column),
            Self::MySQL(m) => m.get_portable_table_column_definition(table_column),
        }
    }

    fn get_portable_table_definition(&self, table: &Row) -> R<String> {
        match self {
            Self::SQLite(m) => m.get_portable_table_definition(table),
            Self::MySQL(m) => m.get_portable_table_definition(table),
        }
    }

    fn get_portable_view_definition(&self, view: &Row) -> R<View> {
        match self {
            Self::SQLite(m) => m.get_portable_view_definition(view),
            Self::MySQL(m) => m.get_portable_view_definition(view),
        }
    }

    fn get_portable_table_foreign_key_definition(
        &self,
        table_foreign_key: &Row,
    ) -> R<ForeignKeyConstraint> {
        match self {
            Self::SQLite(m) => m.get_portable_table_foreign_key_definition(table_foreign_key),
            Self::MySQL(m) => m.get_portable_table_foreign_key_definition(table_foreign_key),
        }
    }

    async fn list_databases(&self) -> R<Vec<String>> {
        match self {
            Self::SQLite(m) => m.list_databases().await,
            Self::MySQL(m) => m.list_databases().await,
        }
    }

    async fn list_schema_names(&self) -> R<Vec<String>> {
        match self {
            Self::SQLite(m) => m.list_schema_names().await,
            Self::MySQL(m) => m.list_schema_names().await,
        }
    }

    async fn list_sequences(&self) -> R<Vec<Sequence>> {
        match self {
            Self::SQLite(m) => m.list_sequences().await,
            Self::MySQL(m) => m.list_sequences().await,
        }
    }

    async fn list_table_columns(&self, table: &str) -> R<ArrayMap<Column>> {
        match self {
            Self::SQLite(m) => m.list_table_columns(table).await,
            Self::MySQL(m) => m.list_table_columns(table).await,
        }
    }

    async fn list_table_indexes(&self, table: &str) -> R<ArrayMap<Index>> {
        match self {
            Self::SQLite(m) => m.list_table_indexes(table).await,
            Self::MySQL(m) => m.list_table_indexes(table).await,
        }
    }

    async fn tables_exist(&self, names: &[&str]) -> R<bool> {
        match self {
            Self::SQLite(m) => m.tables_exist(names).await,
            Self::MySQL(m) => m.tables_exist(names).await,
        }
    }

    async fn table_exists(&self, table_name: &str) -> R<bool> {
        match self {
            Self::SQLite(m) => m.table_exists(table_name).await,
            Self::MySQL(m) => m.table_exists(table_name).await,
        }
    }

    async fn list_table_names(&self) -> R<Vec<String>> {
        match self {
            Self::SQLite(m) => m.list_table_names().await,
            Self::MySQL(m) => m.list_table_names().await,
        }
    }

    async fn list_tables(&self) -> R<Vec<Table>> {
        match self {
            Self::SQLite(m) => m.list_tables().await,
            Self::MySQL(m) => m.list_tables().await,
        }
    }

    async fn get_current_schema_name(&self) -> R<Option<String>> {
        match self {
            Self::SQLite(m) => m.get_current_schema_name().await,
            Self::MySQL(m) => m.get_current_schema_name().await,
        }
    }

    async fn determine_current_schema_name(&self) -> R<Option<String>> {
        match self {
            Self::SQLite(m) => m.determine_current_schema_name().await,
            Self::MySQL(m) => m.determine_current_schema_name().await,
        }
    }

    fn normalize_name(&self, name: &str) -> String {
        match self {
            Self::SQLite(m) => m.normalize_name(name),
            Self::MySQL(m) => m.normalize_name(name),
        }
    }

    fn validate_table_name(&self, input: &str, method_name: &str) {
        match self {
            Self::SQLite(m) => m.validate_table_name(input, method_name),
            Self::MySQL(m) => m.validate_table_name(input, method_name),
        }
    }

    async fn fetch_table_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        match self {
            Self::SQLite(m) => m.fetch_table_columns(database_name, table_name).await,
            Self::MySQL(m) => m.fetch_table_columns(database_name, table_name).await,
        }
    }

    async fn fetch_index_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        match self {
            Self::SQLite(m) => m.fetch_index_columns(database_name, table_name).await,
            Self::MySQL(m) => m.fetch_index_columns(database_name, table_name).await,
        }
    }

    async fn fetch_foreign_key_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        match self {
            Self::SQLite(m) => m.fetch_foreign_key_columns(database_name, table_name).await,
            Self::MySQL(m) => m.fetch_foreign_key_columns(database_name, table_name).await,
        }
    }

    async fn fetch_table_columns_by_table(&self, database_name: &str) -> R<ArrayMap<Vec<Row>>> {
        match self {
            Self::SQLite(m) => m.fetch_table_columns_by_table(database_name).await,
            Self::MySQL(m) => m.fetch_table_columns_by_table(database_name).await,
        }
    }

    async fn fetch_index_columns_by_table(&self, database_name: &str) -> R<ArrayMap<Vec<Row>>> {
        match self {
            Self::SQLite(m) => m.fetch_index_columns_by_table(database_name).await,
            Self::MySQL(m) => m.fetch_index_columns_by_table(database_name).await,
        }
    }

    async fn fetch_foreign_key_columns_by_table(
        &self,
        database_name: &str,
    ) -> R<ArrayMap<Vec<Row>>> {
        match self {
            Self::SQLite(m) => m.fetch_foreign_key_columns_by_table(database_name).await,
            Self::MySQL(m) => m.fetch_foreign_key_columns_by_table(database_name).await,
        }
    }

    async fn introspect_table(&self, name: &str) -> R<Table> {
        match self {
            Self::SQLite(m) => m.introspect_table(name).await,
            Self::MySQL(m) => m.introspect_table(name).await,
        }
    }

    async fn list_views(&self) -> R<Vec<View>> {
        match self {
            Self::SQLite(m) => m.list_views().await,
            Self::MySQL(m) => m.list_views().await,
        }
    }

    async fn list_table_foreign_keys(&self, table: &str) -> R<Vec<ForeignKeyConstraint>> {
        match self {
            Self::SQLite(m) => m.list_table_foreign_keys(table).await,
            Self::MySQL(m) => m.list_table_foreign_keys(table).await,
        }
    }

    async fn get_table_options(&self, name: &str) -> R<PhpArray> {
        match self {
            Self::SQLite(m) => m.get_table_options(name).await,
            Self::MySQL(m) => m.get_table_options(name).await,
        }
    }

    async fn introspect_database_names(&self) -> R<Vec<UnqualifiedName>> {
        match self {
            Self::SQLite(m) => m.introspect_database_names().await,
            Self::MySQL(m) => m.introspect_database_names().await,
        }
    }

    async fn introspect_schema_names(&self) -> R<Vec<UnqualifiedName>> {
        match self {
            Self::SQLite(m) => m.introspect_schema_names().await,
            Self::MySQL(m) => m.introspect_schema_names().await,
        }
    }

    async fn introspect_table_names(&self) -> R<Vec<OptionallyQualifiedName>> {
        match self {
            Self::SQLite(m) => m.introspect_table_names().await,
            Self::MySQL(m) => m.introspect_table_names().await,
        }
    }

    async fn introspect_tables(&self) -> R<Vec<Table>> {
        match self {
            Self::SQLite(m) => m.introspect_tables().await,
            Self::MySQL(m) => m.introspect_tables().await,
        }
    }

    async fn introspect_table0(&self, table_name: &OptionallyQualifiedName) -> R<Table> {
        match self {
            Self::SQLite(m) => m.introspect_table0(table_name).await,
            Self::MySQL(m) => m.introspect_table0(table_name).await,
        }
    }

    async fn introspect_table_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Table> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_by_unquoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_by_unquoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Table> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_by_quoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_by_quoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_columns(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<Column>> {
        match self {
            Self::SQLite(m) => m.introspect_table_columns(table_name).await,
            Self::MySQL(m) => m.introspect_table_columns(table_name).await,
        }
    }

    async fn introspect_table_columns_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Column>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_columns_by_unquoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_columns_by_unquoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_columns_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Column>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_columns_by_quoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_columns_by_quoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_indexes(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<Index>> {
        match self {
            Self::SQLite(m) => m.introspect_table_indexes(table_name).await,
            Self::MySQL(m) => m.introspect_table_indexes(table_name).await,
        }
    }

    async fn introspect_table_indexes_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Index>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_indexes_by_unquoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_indexes_by_unquoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_indexes_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Index>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_indexes_by_quoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_indexes_by_quoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_primary_key_constraint(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Option<PrimaryKeyConstraint>> {
        match self {
            Self::SQLite(m) => m.introspect_table_primary_key_constraint(table_name).await,
            Self::MySQL(m) => m.introspect_table_primary_key_constraint(table_name).await,
        }
    }

    async fn introspect_table_unique_constraints(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<UniqueConstraint>> {
        match self {
            Self::SQLite(m) => m.introspect_table_unique_constraints(table_name).await,
            Self::MySQL(m) => m.introspect_table_unique_constraints(table_name).await,
        }
    }

    async fn introspect_table_unique_constraints_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<UniqueConstraint>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_unique_constraints_by_unquoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_unique_constraints_by_unquoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_unique_constraints_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<UniqueConstraint>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_unique_constraints_by_quoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_unique_constraints_by_quoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_foreign_key_constraints(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<ForeignKeyConstraint>> {
        match self {
            Self::SQLite(m) => m.introspect_table_foreign_key_constraints(table_name).await,
            Self::MySQL(m) => m.introspect_table_foreign_key_constraints(table_name).await,
        }
    }

    async fn introspect_table_foreign_key_constraints_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<ForeignKeyConstraint>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_foreign_key_constraints_by_unquoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_foreign_key_constraints_by_unquoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspect_table_foreign_key_constraints_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<ForeignKeyConstraint>> {
        match self {
            Self::SQLite(m) => {
                m.introspect_table_foreign_key_constraints_by_quoted_name(table_name, schema_name)
                    .await
            }
            Self::MySQL(m) => {
                m.introspect_table_foreign_key_constraints_by_quoted_name(table_name, schema_name)
                    .await
            }
        }
    }

    async fn introspected_names(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<(Option<String>, String)> {
        match self {
            Self::SQLite(m) => m.introspected_names(table_name).await,
            Self::MySQL(m) => m.introspected_names(table_name).await,
        }
    }

    async fn introspect_table_options(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Option<PhpArray>> {
        match self {
            Self::SQLite(m) => m.introspect_table_options(table_name).await,
            Self::MySQL(m) => m.introspect_table_options(table_name).await,
        }
    }

    async fn introspect_views(&self) -> R<Vec<View>> {
        match self {
            Self::SQLite(m) => m.introspect_views().await,
            Self::MySQL(m) => m.introspect_views().await,
        }
    }

    async fn introspect_sequences(&self) -> R<Vec<Sequence>> {
        match self {
            Self::SQLite(m) => m.introspect_sequences().await,
            Self::MySQL(m) => m.introspect_sequences().await,
        }
    }

    async fn drop_database(&self, database: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_database(database).await,
            Self::MySQL(m) => m.drop_database(database).await,
        }
    }

    async fn drop_schema(&self, schema_name: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_schema(schema_name).await,
            Self::MySQL(m) => m.drop_schema(schema_name).await,
        }
    }

    async fn drop_table(&self, name: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_table(name).await,
            Self::MySQL(m) => m.drop_table(name).await,
        }
    }

    async fn drop_index(&self, index: &str, table: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_index(index, table).await,
            Self::MySQL(m) => m.drop_index(index, table).await,
        }
    }

    async fn drop_foreign_key(&self, name: &str, table: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_foreign_key(name, table).await,
            Self::MySQL(m) => m.drop_foreign_key(name, table).await,
        }
    }

    async fn drop_sequence(&self, name: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_sequence(name).await,
            Self::MySQL(m) => m.drop_sequence(name).await,
        }
    }

    async fn drop_unique_constraint(&self, name: &str, table_name: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_unique_constraint(name, table_name).await,
            Self::MySQL(m) => m.drop_unique_constraint(name, table_name).await,
        }
    }

    async fn drop_view(&self, name: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_view(name).await,
            Self::MySQL(m) => m.drop_view(name).await,
        }
    }

    async fn create_schema_objects(&self, schema: &Schema) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_schema_objects(schema).await,
            Self::MySQL(m) => m.create_schema_objects(schema).await,
        }
    }

    async fn create_database(&self, database: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_database(database).await,
            Self::MySQL(m) => m.create_database(database).await,
        }
    }

    async fn create_table(&self, table: &Table) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_table(table).await,
            Self::MySQL(m) => m.create_table(table).await,
        }
    }

    async fn create_sequence(&self, sequence: &Sequence) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_sequence(sequence).await,
            Self::MySQL(m) => m.create_sequence(sequence).await,
        }
    }

    async fn create_index(&self, index: &Index, table: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_index(index, table).await,
            Self::MySQL(m) => m.create_index(index, table).await,
        }
    }

    async fn create_foreign_key(&self, foreign_key: &ForeignKeyConstraint, table: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_foreign_key(foreign_key, table).await,
            Self::MySQL(m) => m.create_foreign_key(foreign_key, table).await,
        }
    }

    async fn create_unique_constraint(
        &self,
        unique_constraint: &UniqueConstraint,
        table_name: &str,
    ) -> R<()> {
        match self {
            Self::SQLite(m) => {
                m.create_unique_constraint(unique_constraint, table_name)
                    .await
            }
            Self::MySQL(m) => {
                m.create_unique_constraint(unique_constraint, table_name)
                    .await
            }
        }
    }

    async fn create_view(&self, view: &View) -> R<()> {
        match self {
            Self::SQLite(m) => m.create_view(view).await,
            Self::MySQL(m) => m.create_view(view).await,
        }
    }

    async fn drop_schema_objects(&self, schema: &Schema) -> R<()> {
        match self {
            Self::SQLite(m) => m.drop_schema_objects(schema).await,
            Self::MySQL(m) => m.drop_schema_objects(schema).await,
        }
    }

    async fn alter_schema(&self, schema_diff: &SchemaDiff) -> R<()> {
        match self {
            Self::SQLite(m) => m.alter_schema(schema_diff).await,
            Self::MySQL(m) => m.alter_schema(schema_diff).await,
        }
    }

    async fn alter_table(&self, table_diff: &TableDiff) -> R<()> {
        match self {
            Self::SQLite(m) => m.alter_table(table_diff).await,
            Self::MySQL(m) => m.alter_table(table_diff).await,
        }
    }

    async fn rename_table(&self, name: &str, new_name: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.rename_table(name, new_name).await,
            Self::MySQL(m) => m.rename_table(name, new_name).await,
        }
    }

    fn get_portable_database_definition(&self, database: &Row) -> R<String> {
        match self {
            Self::SQLite(m) => m.get_portable_database_definition(database),
            Self::MySQL(m) => m.get_portable_database_definition(database),
        }
    }

    fn get_portable_sequence_definition(&self, sequence: &Row) -> R<Sequence> {
        match self {
            Self::SQLite(m) => m.get_portable_sequence_definition(sequence),
            Self::MySQL(m) => m.get_portable_sequence_definition(sequence),
        }
    }

    fn get_portable_table_column_list(
        &self,
        table: &str,
        database: &str,
        rows: &[Row],
    ) -> R<ArrayMap<Column>> {
        match self {
            Self::SQLite(m) => m.get_portable_table_column_list(table, database, rows),
            Self::MySQL(m) => m.get_portable_table_column_list(table, database, rows),
        }
    }

    fn get_portable_table_indexes_list(
        &self,
        rows: &[Row],
        table_name: &str,
    ) -> R<ArrayMap<Index>> {
        match self {
            Self::SQLite(m) => m.get_portable_table_indexes_list(rows, table_name),
            Self::MySQL(m) => m.get_portable_table_indexes_list(rows, table_name),
        }
    }

    async fn get_portable_table_foreign_keys_list(
        &self,
        rows: &[Row],
    ) -> R<Vec<ForeignKeyConstraint>> {
        match self {
            Self::SQLite(m) => m.get_portable_table_foreign_keys_list(rows).await,
            Self::MySQL(m) => m.get_portable_table_foreign_keys_list(rows).await,
        }
    }

    async fn execute_statements(&self, sql: &[String]) -> R<()> {
        match self {
            Self::SQLite(m) => m.execute_statements(sql).await,
            Self::MySQL(m) => m.execute_statements(sql).await,
        }
    }

    async fn execute_statement(&self, sql: &str) -> R<()> {
        match self {
            Self::SQLite(m) => m.execute_statement(sql).await,
            Self::MySQL(m) => m.execute_statement(sql).await,
        }
    }

    async fn introspect_schema(&self) -> R<Schema> {
        match self {
            Self::SQLite(m) => m.introspect_schema().await,
            Self::MySQL(m) => m.introspect_schema().await,
        }
    }

    async fn create_schema_config(&self) -> R<SchemaConfig> {
        match self {
            Self::SQLite(m) => m.create_schema_config().await,
            Self::MySQL(m) => m.create_schema_config().await,
        }
    }

    async fn get_database(&self, method_name: &str) -> R<String> {
        match self {
            Self::SQLite(m) => m.get_database(method_name).await,
            Self::MySQL(m) => m.get_database(method_name).await,
        }
    }

    fn group_by_table(&self, rows: Vec<Row>) -> R<ArrayMap<Vec<Row>>> {
        match self {
            Self::SQLite(m) => m.group_by_table(rows),
            Self::MySQL(m) => m.group_by_table(rows),
        }
    }
}
