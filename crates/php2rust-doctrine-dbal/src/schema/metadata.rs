//! Port of Doctrine\DBAL\Schema\Metadata: the rows of database object
//! metadata that a [`MetadataProvider`] gives the schema introspection.
//!
//! PHP's provider methods return lazy iterables; here they return the
//! rows at once (the introspection reads every row before it builds an
//! object, so nothing observable changes but when a query runs).
//! `getUniqueConstraintColumnsForAllTables()` and
//! `getUniqueConstraintColumnsForTable()` are not part of PHP's
//! interface (`IntrospectingSchemaProvider` tests for them with
//! `method_exists()`): here they are provided methods that answer
//! `None`, "not implemented".
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Metadata/MetadataProvider.php src/Schema/Metadata/DatabaseMetadataRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/ForeignKeyConstraintColumnMetadataRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/IndexColumnMetadataRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/PrimaryKeyConstraintColumnRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/SchemaMetadataRow.php src/Schema/Metadata/SequenceMetadataRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/TableColumnMetadataRow.php src/Schema/Metadata/TableMetadataRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/UniqueConstraintColumnMetadataRow.php
//! Upstream: doctrine/dbal src/Schema/Metadata/ViewMetadataRow.php

use php_value::PhpArray;

use crate::exception::Exception;

use super::column::Column;
use super::foreign_key_constraint::{MatchType, ReferentialAction};
use super::index::IndexType;

/// The `int|string` id of a constraint's rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RowId {
    Int(i64),
    String(String),
}

/// `DatabaseMetadataRow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseMetadataRow {
    database_name: String,
}

impl DatabaseMetadataRow {
    pub fn new(database_name: &str) -> Self {
        Self {
            database_name: database_name.to_owned(),
        }
    }

    pub fn get_database_name(&self) -> &str {
        &self.database_name
    }
}

/// `SchemaMetadataRow`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaMetadataRow {
    schema_name: String,
}

impl SchemaMetadataRow {
    pub fn new(schema_name: &str) -> Self {
        Self {
            schema_name: schema_name.to_owned(),
        }
    }

    pub fn get_schema_name(&self) -> &str {
        &self.schema_name
    }
}

/// `TableMetadataRow`: a table's name and options.
#[derive(Debug, Clone)]
pub struct TableMetadataRow {
    schema_name: Option<String>,
    table_name: String,
    options: PhpArray,
}

impl TableMetadataRow {
    pub fn new(schema_name: Option<&str>, table_name: &str, options: PhpArray) -> Self {
        Self {
            schema_name: schema_name.map(str::to_owned),
            table_name: table_name.to_owned(),
            options,
        }
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
    }

    pub fn get_options(&self) -> &PhpArray {
        &self.options
    }
}

/// `TableColumnMetadataRow`: a column of a table.
#[derive(Debug, Clone)]
pub struct TableColumnMetadataRow {
    schema_name: Option<String>,
    table_name: String,
    column: Column,
}

impl TableColumnMetadataRow {
    pub fn new(schema_name: Option<&str>, table_name: &str, column: Column) -> Self {
        Self {
            schema_name: schema_name.map(str::to_owned),
            table_name: table_name.to_owned(),
            column,
        }
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
    }

    pub fn get_column(&self) -> &Column {
        &self.column
    }

    pub(crate) fn into_column(self) -> Column {
        self.column
    }
}

/// `IndexColumnMetadataRow`: a column of an index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexColumnMetadataRow {
    schema_name: Option<String>,
    table_name: String,
    index_name: String,
    type_: IndexType,
    is_clustered: bool,
    predicate: Option<String>,
    column_name: String,
    column_length: Option<i64>,
}

impl IndexColumnMetadataRow {
    #[allow(clippy::too_many_arguments, reason = "PHP's constructor")]
    pub fn new(
        schema_name: Option<&str>,
        table_name: &str,
        index_name: &str,
        type_: IndexType,
        is_clustered: bool,
        predicate: Option<&str>,
        column_name: &str,
        column_length: Option<i64>,
    ) -> Self {
        Self {
            schema_name: schema_name.map(str::to_owned),
            table_name: table_name.to_owned(),
            index_name: index_name.to_owned(),
            type_,
            is_clustered,
            predicate: predicate.map(str::to_owned),
            column_name: column_name.to_owned(),
            column_length,
        }
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
    }

    pub fn get_index_name(&self) -> &str {
        &self.index_name
    }

    pub fn get_type(&self) -> IndexType {
        self.type_
    }

    pub fn is_clustered(&self) -> bool {
        self.is_clustered
    }

    pub fn get_predicate(&self) -> Option<&str> {
        self.predicate.as_deref()
    }

    pub fn get_column_name(&self) -> &str {
        &self.column_name
    }

    pub fn get_column_length(&self) -> Option<i64> {
        self.column_length
    }
}

/// `PrimaryKeyConstraintColumnRow`: a column of a primary key constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryKeyConstraintColumnRow {
    schema_name: Option<String>,
    table_name: String,
    constraint_name: Option<String>,
    is_clustered: bool,
    column_name: String,
}

impl PrimaryKeyConstraintColumnRow {
    pub fn new(
        schema_name: Option<&str>,
        table_name: &str,
        constraint_name: Option<&str>,
        is_clustered: bool,
        column_name: &str,
    ) -> Self {
        Self {
            schema_name: schema_name.map(str::to_owned),
            table_name: table_name.to_owned(),
            constraint_name: constraint_name.map(str::to_owned),
            is_clustered,
            column_name: column_name.to_owned(),
        }
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
    }

    pub fn get_constraint_name(&self) -> Option<&str> {
        self.constraint_name.as_deref()
    }

    pub fn is_clustered(&self) -> bool {
        self.is_clustered
    }

    pub fn get_column_name(&self) -> &str {
        &self.column_name
    }
}

/// The id of a constraint row: `$id`, or the name when it is null.
fn row_id(id: Option<RowId>, name: Option<&str>) -> Result<RowId, Exception> {
    match (id, name) {
        (Some(id), _) => Ok(id),
        (None, Some(name)) => Ok(RowId::String(name.to_owned())),
        (None, None) => Err(Exception::invalid_argument(
            "Either the id or name must be set to a non-null value.",
        )),
    }
}

/// `UniqueConstraintColumnMetadataRow`: a column of a unique constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniqueConstraintColumnMetadataRow {
    schema_name: Option<String>,
    table_name: String,
    id: RowId,
    name: Option<String>,
    column_name: String,
}

impl UniqueConstraintColumnMetadataRow {
    pub fn new(
        schema_name: Option<&str>,
        table_name: &str,
        id: Option<RowId>,
        name: Option<&str>,
        column_name: &str,
    ) -> Result<Self, Exception> {
        Ok(Self {
            schema_name: schema_name.map(str::to_owned),
            table_name: table_name.to_owned(),
            id: row_id(id, name)?,
            name: name.map(str::to_owned),
            column_name: column_name.to_owned(),
        })
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_table_name(&self) -> &str {
        &self.table_name
    }

    pub fn get_id(&self) -> &RowId {
        &self.id
    }

    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn get_column_name(&self) -> &str {
        &self.column_name
    }
}

/// `ForeignKeyConstraintColumnMetadataRow`: a column pair of a foreign key
/// constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeyConstraintColumnMetadataRow {
    referencing_schema_name: Option<String>,
    referencing_table_name: String,
    id: RowId,
    name: Option<String>,
    referenced_schema_name: Option<String>,
    referenced_table_name: String,
    match_type: MatchType,
    on_update_action: ReferentialAction,
    on_delete_action: ReferentialAction,
    is_deferrable: bool,
    is_deferred: bool,
    referencing_column_name: String,
    referenced_column_name: String,
}

/// The arguments of `new ForeignKeyConstraintColumnMetadataRow(...)`, by
/// PHP's parameter names.
#[derive(Debug, Clone)]
pub struct ForeignKeyConstraintColumnMetadataRowParts<'a> {
    pub referencing_schema_name: Option<&'a str>,
    pub referencing_table_name: &'a str,
    pub id: Option<RowId>,
    pub name: Option<&'a str>,
    pub referenced_schema_name: Option<&'a str>,
    pub referenced_table_name: &'a str,
    pub match_type: MatchType,
    pub on_update_action: ReferentialAction,
    pub on_delete_action: ReferentialAction,
    pub is_deferrable: bool,
    pub is_deferred: bool,
    pub referencing_column_name: &'a str,
    pub referenced_column_name: &'a str,
}

impl ForeignKeyConstraintColumnMetadataRow {
    pub fn new(parts: ForeignKeyConstraintColumnMetadataRowParts<'_>) -> Result<Self, Exception> {
        Ok(Self {
            referencing_schema_name: parts.referencing_schema_name.map(str::to_owned),
            referencing_table_name: parts.referencing_table_name.to_owned(),
            id: row_id(parts.id, parts.name)?,
            name: parts.name.map(str::to_owned),
            referenced_schema_name: parts.referenced_schema_name.map(str::to_owned),
            referenced_table_name: parts.referenced_table_name.to_owned(),
            match_type: parts.match_type,
            on_update_action: parts.on_update_action,
            on_delete_action: parts.on_delete_action,
            is_deferrable: parts.is_deferrable,
            is_deferred: parts.is_deferred,
            referencing_column_name: parts.referencing_column_name.to_owned(),
            referenced_column_name: parts.referenced_column_name.to_owned(),
        })
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.referencing_schema_name.as_deref()
    }

    pub fn get_table_name(&self) -> &str {
        &self.referencing_table_name
    }

    pub fn get_id(&self) -> &RowId {
        &self.id
    }

    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn get_referenced_schema_name(&self) -> Option<&str> {
        self.referenced_schema_name.as_deref()
    }

    pub fn get_referenced_table_name(&self) -> &str {
        &self.referenced_table_name
    }

    pub fn get_match_type(&self) -> MatchType {
        self.match_type
    }

    pub fn get_on_update_action(&self) -> ReferentialAction {
        self.on_update_action
    }

    pub fn get_on_delete_action(&self) -> ReferentialAction {
        self.on_delete_action
    }

    pub fn is_deferrable(&self) -> bool {
        self.is_deferrable
    }

    pub fn is_deferred(&self) -> bool {
        self.is_deferred
    }

    pub fn get_referencing_column_name(&self) -> &str {
        &self.referencing_column_name
    }

    pub fn get_referenced_column_name(&self) -> &str {
        &self.referenced_column_name
    }
}

/// `ViewMetadataRow`: a view's name and definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewMetadataRow {
    schema_name: Option<String>,
    view_name: String,
    definition: String,
}

impl ViewMetadataRow {
    pub fn new(schema_name: Option<&str>, view_name: &str, definition: &str) -> Self {
        Self {
            schema_name: schema_name.map(str::to_owned),
            view_name: view_name.to_owned(),
            definition: definition.to_owned(),
        }
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_view_name(&self) -> &str {
        &self.view_name
    }

    pub fn get_definition(&self) -> &str {
        &self.definition
    }
}

/// `SequenceMetadataRow`: a sequence's name and options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequenceMetadataRow {
    schema_name: Option<String>,
    sequence_name: String,
    allocation_size: i64,
    initial_value: i64,
    cache_size: Option<i64>,
}

impl SequenceMetadataRow {
    pub fn new(
        schema_name: Option<&str>,
        sequence_name: &str,
        allocation_size: i64,
        initial_value: i64,
        cache_size: Option<i64>,
    ) -> Self {
        Self {
            schema_name: schema_name.map(str::to_owned),
            sequence_name: sequence_name.to_owned(),
            allocation_size,
            initial_value,
            cache_size,
        }
    }

    pub fn get_schema_name(&self) -> Option<&str> {
        self.schema_name.as_deref()
    }

    pub fn get_sequence_name(&self) -> &str {
        &self.sequence_name
    }

    pub fn get_allocation_size(&self) -> i64 {
        self.allocation_size
    }

    pub fn get_initial_value(&self) -> i64 {
        self.initial_value
    }

    pub fn get_cache_size(&self) -> Option<i64> {
        self.cache_size
    }
}

type Rows<T> = Result<Vec<T>, Exception>;

/// `MetadataProvider`: provides metadata about database objects.
#[allow(
    async_fn_in_trait,
    reason = "the futures are awaited where they are made"
)]
pub trait MetadataProvider {
    async fn get_all_database_names(&self) -> Rows<DatabaseMetadataRow>;

    async fn get_all_schema_names(&self) -> Rows<SchemaMetadataRow>;

    async fn get_all_table_names(&self) -> Rows<TableMetadataRow>;

    async fn get_table_columns_for_all_tables(&self) -> Rows<TableColumnMetadataRow>;

    async fn get_table_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableColumnMetadataRow>;

    async fn get_index_columns_for_all_tables(&self) -> Rows<IndexColumnMetadataRow>;

    async fn get_index_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<IndexColumnMetadataRow>;

    async fn get_primary_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<PrimaryKeyConstraintColumnRow>;

    async fn get_primary_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<PrimaryKeyConstraintColumnRow>;

    /// `getUniqueConstraintColumnsForAllTables()`, `None` when the
    /// provider does not implement it.
    async fn get_unique_constraint_columns_for_all_tables(
        &self,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        None
    }

    /// `getUniqueConstraintColumnsForTable()`, `None` when the provider
    /// does not implement it.
    async fn get_unique_constraint_columns_for_table(
        &self,
        _schema_name: Option<&str>,
        _table_name: &str,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        None
    }

    async fn get_foreign_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow>;

    async fn get_foreign_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow>;

    async fn get_table_options_for_all_tables(&self) -> Rows<TableMetadataRow>;

    async fn get_table_options_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableMetadataRow>;

    async fn get_all_views(&self) -> Rows<ViewMetadataRow>;

    async fn get_all_sequences(&self) -> Rows<SequenceMetadataRow>;

    /// The class name, for the deprecation messages that name the
    /// provider (`$this->metadataProvider::class`).
    fn class(&self) -> &str;
}
