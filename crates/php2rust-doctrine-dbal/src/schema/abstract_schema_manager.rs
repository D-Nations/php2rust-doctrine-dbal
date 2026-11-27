//! Port of Doctrine\DBAL\Schema\AbstractSchemaManager.php: the base of the
//! schema managers, which inspect and modify the database schema.
//!
//! The abstract class is a trait: its abstract (protected) methods are
//! required methods, its concrete methods are provided ones that a
//! platform's manager overrides as PHP's subclass does. The protected
//! hooks are public trait methods (a test calls `selectTableColumns()`
//! through a subclass). The methods that query the database are `async`;
//! the trait is not object-safe, so a connection hands out the concrete
//! `SQLiteSchemaManager`.
//!
//! Differences: the current schema name is not cached (`SQLite` has no
//! schemas, so it is always `None`); `createComparator()` is the
//! platform manager's own method (`SQLite`'s returns its comparator), and
//! so is `migrateSchema()`, which uses it. Rows are associative
//! `PhpArray`s as `fetchAllAssociative()` gives them.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/AbstractSchemaManager.php

use php_value::{Key, PhpArray, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::{AbstractPlatform, NotSupported};
use crate::{Connection, ParameterTypes};

use super::array_map::ArrayMap;
use super::column::Column;
use super::exception::table_does_not_exist;
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::identifier::Identifier;
use super::index::Index;
use super::introspection::IntrospectingSchemaProvider;
use super::metadata::MetadataProvider;
use super::name::{OptionallyQualifiedName, UnqualifiedName, parse_optionally_qualified_name};
use super::primary_key_constraint::PrimaryKeyConstraint;
use super::schema::Schema;
use super::schema_config::SchemaConfig;
use super::schema_diff::SchemaDiff;
use super::sequence::Sequence;
use super::table::{Table, TableParts};
use super::table_diff::TableDiff;
use super::unique_constraint::UniqueConstraint;
use super::view::View;

const CLASS: &str = "Doctrine\\DBAL\\Schema\\AbstractSchemaManager";
const LINK_TABLE_NAME: &str = "https://github.com/doctrine/dbal/pull/6768";

type R<T> = Result<T, Exception>;

/// A row as `fetchAllAssociative()` gives it.
pub type Row = PhpArray;

/// `__METHOD__` of a method declared by `AbstractSchemaManager`.
pub(crate) fn method(name: &str) -> String {
    format!("{CLASS}::{name}")
}

/// `$row[$key]`.
pub(crate) fn field<'r>(row: &'r Row, key: &str) -> Option<&'r Value> {
    row.get(&Key::from(key))
}

/// `$row[$key]` as a string.
pub(crate) fn field_text(row: &Row, key: &str) -> R<String> {
    crate::platforms::sqlite::text_value(field(row, key))
}

/// `isset($row[$key])`.
pub(crate) fn isset(row: &Row, key: &str) -> bool {
    field(row, key).is_some_and(|v| !v.is_null())
}

/// `strtolower()` (ASCII).
fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// Base trait for schema managers. Schema managers are used to inspect
/// and/or modify the database schema/structure.
#[allow(
    async_fn_in_trait,
    reason = "the futures are awaited where they are made"
)]
pub trait AbstractSchemaManager<'c> {
    /// The metadata provider of the platform.
    type MetadataProvider: MetadataProvider;

    /// `$this->connection`.
    fn connection(&self) -> &'c Connection;

    /// `$this->platform`.
    fn platform(&self) -> &'c dyn AbstractPlatform;

    /// `$this->platform->createMetadataProvider($this->connection)`. A
    /// provider may query the database when it is made (`MySQL`'s reads
    /// the current database name).
    async fn create_metadata_provider(&self) -> R<Self::MetadataProvider>;

    // The abstract (protected) methods.

    /// Selects names of tables in the specified database.
    async fn select_table_names(&self, database_name: &str) -> R<crate::Result<'c>>;

    /// Selects definitions of table columns in the specified database. If
    /// the table name is specified, narrows down the selection to this
    /// table.
    async fn select_table_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>>;

    /// Selects definitions of index columns in the specified database.
    async fn select_index_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>>;

    /// Selects definitions of foreign key columns in the specified
    /// database.
    async fn select_foreign_key_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>>;

    /// Fetches table options for the tables in the specified database and
    /// returns them grouped by table name.
    async fn fetch_table_options_by_table(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<ArrayMap<PhpArray>>;

    /// Gets Table Column Definition.
    fn get_portable_table_column_definition(&self, table_column: &Row) -> R<Column>;

    /// `_getPortableTableDefinition($table)` (deprecated).
    fn get_portable_table_definition(&self, table: &Row) -> R<String>;

    /// `_getPortableViewDefinition($view)`.
    fn get_portable_view_definition(&self, view: &Row) -> R<View>;

    /// `_getPortableTableForeignKeyDefinition($tableForeignKey)`.
    fn get_portable_table_foreign_key_definition(
        &self,
        table_foreign_key: &Row,
    ) -> R<ForeignKeyConstraint>;

    // The concrete methods.

    /// Lists the available databases for this connection (deprecated).
    async fn list_databases(&self) -> R<Vec<String>> {
        let rows = self
            .connection()
            .fetch_all_associative(
                &self.platform().get_list_databases_sql()?,
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?;
        rows.iter()
            .map(|row| self.get_portable_database_definition(row))
            .collect()
    }

    /// Returns a list of the names of all schemata in the current
    /// database (deprecated).
    async fn list_schema_names(&self) -> R<Vec<String>> {
        Err(NotSupported::new(&method("listSchemaNames")).into())
    }

    /// Lists the available sequences for this connection (deprecated).
    async fn list_sequences(&self) -> R<Vec<Sequence>> {
        let database = self.get_database("listSequences").await?;
        let rows = self
            .connection()
            .fetch_all_associative(
                &self.platform().get_list_sequences_sql(&database)?,
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?;
        let mut sequences = Vec::new();
        for row in &rows {
            let sequence = self.get_portable_sequence_definition(row)?;
            if (self
                .connection()
                .get_configuration()
                .get_schema_assets_filter())(&sequence.get_name())
            {
                sequences.push(sequence);
            }
        }
        Ok(sequences)
    }

    /// Lists the columns for a given table (deprecated).
    async fn list_table_columns(&self, table: &str) -> R<ArrayMap<Column>> {
        self.validate_table_name(table, "listTableColumns");

        let database = self.get_database("listTableColumns").await?;

        let rows = self
            .fetch_table_columns(&database, Some(&self.normalize_name(table)))
            .await?;
        self.get_portable_table_column_list(table, &database, &rows)
    }

    /// Lists the indexes for a given table returning an array of Index
    /// instances (deprecated). Keys of the portable indexes list are all
    /// lower-cased.
    async fn list_table_indexes(&self, table: &str) -> R<ArrayMap<Index>> {
        self.validate_table_name(table, "listTableIndexes");

        let database = self.get_database("listTableIndexes").await?;
        let table = self.normalize_name(table);

        let rows = self.fetch_index_columns(&database, Some(&table)).await?;
        self.get_portable_table_indexes_list(&rows, &table)
    }

    /// Returns true if all the given tables exist.
    async fn tables_exist(&self, names: &[&str]) -> R<bool> {
        let names: Vec<String> = names.iter().map(|n| lower(n)).collect();
        let existing: Vec<String> = self
            .list_table_names()
            .await?
            .iter()
            .map(|n| lower(n))
            .collect();

        Ok(names.len() == names.iter().filter(|n| existing.contains(n)).count())
    }

    async fn table_exists(&self, table_name: &str) -> R<bool> {
        self.tables_exist(&[table_name]).await
    }

    /// Returns a list of all tables in the current database (deprecated).
    async fn list_table_names(&self) -> R<Vec<String>> {
        let database = self.get_database("listTableNames").await?;
        let rows = self
            .select_table_names(&database)
            .await?
            .fetch_all_associative()?;
        let filter = self
            .connection()
            .get_configuration()
            .get_schema_assets_filter()
            .clone();
        let mut names = Vec::new();
        for row in &rows {
            let name = self.get_portable_table_definition(row)?;
            if filter(&name) {
                names.push(name);
            }
        }
        Ok(names)
    }

    /// Lists the tables for this connection (deprecated).
    async fn list_tables(&self) -> R<Vec<Table>> {
        let database = self.get_database("listTables").await?;

        let table_columns_by_table = self.fetch_table_columns_by_table(&database).await?;
        let index_columns_by_table = self.fetch_index_columns_by_table(&database).await?;
        let foreign_key_columns_by_table =
            self.fetch_foreign_key_columns_by_table(&database).await?;
        let table_options_by_table = self.fetch_table_options_by_table(&database, None).await?;

        let filter = self
            .connection()
            .get_configuration()
            .get_schema_assets_filter()
            .clone();
        let mut tables = Vec::new();

        let configuration = self.create_schema_config().await?.to_table_configuration();

        for (table_name, table_columns) in table_columns_by_table.iter() {
            if filter(table_name) {
                let columns =
                    self.get_portable_table_column_list(table_name, &database, table_columns)?;
                let indexes = self.get_portable_table_indexes_list(
                    index_columns_by_table
                        .get(table_name)
                        .map_or(&[][..], Vec::as_slice),
                    table_name,
                )?;
                let foreign_keys = self
                    .get_portable_table_foreign_keys_list(
                        foreign_key_columns_by_table
                            .get(table_name)
                            .map_or(&[][..], Vec::as_slice),
                    )
                    .await?;
                tables.push(Table::with_parts(
                    table_name,
                    TableParts {
                        columns: columns.values().cloned().collect(),
                        indexes: indexes.values().cloned().collect(),
                        fk_constraints: foreign_keys,
                        options: table_options_by_table
                            .get(table_name)
                            .cloned()
                            .unwrap_or_default(),
                        configuration: Some(configuration),
                        ..TableParts::default()
                    },
                )?);
            }
        }

        Ok(tables)
    }

    /// Returns the current schema name used by the schema manager
    /// connection (`None`: no schema is selected, or the platform does
    /// not support schemas).
    async fn get_current_schema_name(&self) -> R<Option<String>> {
        if !self.platform().supports_schemas() {
            return Ok(None);
        }

        self.determine_current_schema_name().await
    }

    /// Determines the name of the current schema.
    async fn determine_current_schema_name(&self) -> R<Option<String>> {
        Err(NotSupported::new(&method("determineCurrentSchemaName")).into())
    }

    /// An extension point for those platforms where case sensitivity of
    /// the object name depends on whether it's quoted (deprecated).
    fn normalize_name(&self, name: &str) -> String {
        Identifier::new(name, false).get_name()
    }

    /// `validateTableName($input, $methodName)`.
    fn validate_table_name(&self, input: &str, method_name: &str) {
        let method_name = method(method_name);
        match parse_optionally_qualified_name(input) {
            Err(e) => {
                let message = e.message().to_string_lossy().into_owned();
                deprecations::trigger(LINK_TABLE_NAME, move || {
                    format!("Unable to parse table name passed to {method_name}(): {message}.")
                });
            }
            Ok(table_name) => {
                if table_name.get_qualifier().is_some() && !self.platform().supports_schemas() {
                    let class = self.platform().class().to_owned();
                    deprecations::trigger(LINK_TABLE_NAME, move || {
                        format!(
                            "Relying on {method_name}() not parsing an unquoted table name containing a dot while working with {class} is deprecated. Pass a quoted name instead."
                        )
                    });
                }
            }
        }
    }

    /// Fetches definitions of table columns in the specified database.
    async fn fetch_table_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        self.select_table_columns(database_name, table_name)
            .await?
            .fetch_all_associative()
    }

    /// Fetches definitions of index columns in the specified database.
    async fn fetch_index_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        self.select_index_columns(database_name, table_name)
            .await?
            .fetch_all_associative()
    }

    /// Fetches definitions of foreign key columns in the specified
    /// database.
    async fn fetch_foreign_key_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        self.select_foreign_key_columns(database_name, table_name)
            .await?
            .fetch_all_associative()
    }

    /// Fetches definitions of table columns and returns them grouped by
    /// table name.
    async fn fetch_table_columns_by_table(&self, database_name: &str) -> R<ArrayMap<Vec<Row>>> {
        let rows = self.fetch_table_columns(database_name, None).await?;
        self.group_by_table(rows)
    }

    /// Fetches definitions of index columns and returns them grouped by
    /// table name.
    async fn fetch_index_columns_by_table(&self, database_name: &str) -> R<ArrayMap<Vec<Row>>> {
        let rows = self.fetch_index_columns(database_name, None).await?;
        self.group_by_table(rows)
    }

    /// Fetches definitions of foreign key columns and returns them grouped
    /// by table name.
    async fn fetch_foreign_key_columns_by_table(
        &self,
        database_name: &str,
    ) -> R<ArrayMap<Vec<Row>>> {
        let rows = self.fetch_foreign_key_columns(database_name, None).await?;
        self.group_by_table(rows)
    }

    /// Introspects the table with the given name (deprecated).
    async fn introspect_table(&self, name: &str) -> R<Table> {
        let columns = self.list_table_columns(name).await?;

        if columns.is_empty() {
            return Err(table_does_not_exist(name));
        }

        let indexes = self.list_table_indexes(name).await?;
        let foreign_keys = self.list_table_foreign_keys(name).await?;
        let options = self.get_table_options(name).await?;

        Table::with_parts(
            name,
            TableParts {
                columns: columns.values().cloned().collect(),
                indexes: indexes.values().cloned().collect(),
                fk_constraints: foreign_keys,
                options,
                ..TableParts::default()
            },
        )
    }

    /// Lists the views this connection has (deprecated).
    async fn list_views(&self) -> R<Vec<View>> {
        let database = self.get_database("listViews").await?;
        let rows = self
            .connection()
            .fetch_all_associative(
                &self.platform().get_list_views_sql(&database)?,
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?;
        rows.iter()
            .map(|row| self.get_portable_view_definition(row))
            .collect()
    }

    /// Lists the foreign keys for the given table (deprecated).
    async fn list_table_foreign_keys(&self, table: &str) -> R<Vec<ForeignKeyConstraint>> {
        self.validate_table_name(table, "listTableForeignKeys");

        let database = self.get_database("listTableForeignKeys").await?;

        let rows = self
            .fetch_foreign_key_columns(&database, Some(&self.normalize_name(table)))
            .await?;
        self.get_portable_table_foreign_keys_list(&rows).await
    }

    /// `getTableOptions($name)` (private).
    async fn get_table_options(&self, name: &str) -> R<PhpArray> {
        self.validate_table_name(name, "getTableOptions");

        let normalized_name = self.normalize_name(name);

        let database = self.get_database("getTableOptions").await?;
        Ok(self
            .fetch_table_options_by_table(&database, Some(&normalized_name))
            .await?
            .get(&normalized_name)
            .cloned()
            .unwrap_or_default())
    }

    /// Introspects available databases and returns their names.
    async fn introspect_database_names(&self) -> R<Vec<UnqualifiedName>> {
        self.create_schema_provider()
            .await?
            .get_all_database_names()
            .await
    }

    /// Introspects schemas in the current database and returns their
    /// names.
    async fn introspect_schema_names(&self) -> R<Vec<UnqualifiedName>> {
        self.create_schema_provider()
            .await?
            .get_all_schema_names()
            .await
    }

    /// Introspects tables in the current database and returns their names.
    async fn introspect_table_names(&self) -> R<Vec<OptionallyQualifiedName>> {
        let filter = self
            .connection()
            .get_configuration()
            .get_schema_assets_filter()
            .clone();
        Ok(self
            .create_schema_provider()
            .await?
            .get_all_table_names()
            .await?
            .into_iter()
            .filter(|name| filter(&formatted_name(name)))
            .collect())
    }

    /// Introspects tables in the current database and returns their
    /// definitions.
    async fn introspect_tables(&self) -> R<Vec<Table>> {
        let filter = self
            .connection()
            .get_configuration()
            .get_schema_assets_filter()
            .clone();
        let mut tables = Vec::new();
        for table in self
            .create_schema_provider()
            .await?
            .get_all_tables()
            .await?
        {
            if filter(&formatted_name(&table.get_object_name()?)) {
                tables.push(table);
            }
        }
        Ok(tables)
    }

    /// `introspectTable0($tableName)` (private).
    async fn introspect_table0(&self, table_name: &OptionallyQualifiedName) -> R<Table> {
        let columns = self.introspect_table_columns(table_name).await?;

        if columns.is_empty() {
            return Err(table_does_not_exist(&table_name.to_string()));
        }

        let Some(options) = self.introspect_table_options(table_name).await? else {
            return Err(crate::platforms::sqlite::assertion_error(
                "$options !== null",
            ));
        };

        Table::editor()
            .set_name(table_name.clone())
            .set_columns(columns)
            .set_primary_key_constraint(
                self.introspect_table_primary_key_constraint(table_name)
                    .await?,
            )
            .set_indexes(self.introspect_table_indexes(table_name).await?)
            .set_unique_constraints(self.introspect_table_unique_constraints(table_name).await?)
            .set_foreign_key_constraints(
                self.introspect_table_foreign_key_constraints(table_name)
                    .await?,
            )
            .set_options(options)
            .create()
    }

    /// Introspects the table with the given unquoted name and schema name.
    async fn introspect_table_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Table> {
        self.introspect_table0(&OptionallyQualifiedName::unquoted(table_name, schema_name)?)
            .await
    }

    /// Introspects the table with the given quoted name and schema name.
    async fn introspect_table_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Table> {
        self.introspect_table0(&OptionallyQualifiedName::quoted(table_name, schema_name)?)
            .await
    }

    /// Introspects the columns of a given table (empty if the table does
    /// not exist).
    async fn introspect_table_columns(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<Column>> {
        let (schema_name, table_name) = self.introspected_names(table_name).await?;
        self.create_schema_provider()
            .await?
            .get_columns_for_table(schema_name.as_deref(), &table_name)
            .await
    }

    async fn introspect_table_columns_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Column>> {
        self.introspect_table_columns(&OptionallyQualifiedName::unquoted(table_name, schema_name)?)
            .await
    }

    async fn introspect_table_columns_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Column>> {
        self.introspect_table_columns(&OptionallyQualifiedName::quoted(table_name, schema_name)?)
            .await
    }

    /// Introspects the indexes of a given table.
    async fn introspect_table_indexes(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<Index>> {
        let (schema_name, table_name) = self.introspected_names(table_name).await?;
        self.create_schema_provider()
            .await?
            .get_indexes_for_table(schema_name.as_deref(), &table_name)
            .await
    }

    async fn introspect_table_indexes_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Index>> {
        self.introspect_table_indexes(&OptionallyQualifiedName::unquoted(table_name, schema_name)?)
            .await
    }

    async fn introspect_table_indexes_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<Index>> {
        self.introspect_table_indexes(&OptionallyQualifiedName::quoted(table_name, schema_name)?)
            .await
    }

    /// Introspects the primary key constraint of a given table.
    async fn introspect_table_primary_key_constraint(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Option<PrimaryKeyConstraint>> {
        let (schema_name, table_name) = self.introspected_names(table_name).await?;
        self.create_schema_provider()
            .await?
            .get_primary_key_constraint_for_table(schema_name.as_deref(), &table_name)
            .await
    }

    /// Introspects the unique constraints of a given table.
    async fn introspect_table_unique_constraints(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<UniqueConstraint>> {
        let (schema_name, table_name) = self.introspected_names(table_name).await?;
        self.create_schema_provider()
            .await?
            .get_unique_constraints_for_table(schema_name.as_deref(), &table_name)
            .await
    }

    async fn introspect_table_unique_constraints_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<UniqueConstraint>> {
        self.introspect_table_unique_constraints(&OptionallyQualifiedName::unquoted(
            table_name,
            schema_name,
        )?)
        .await
    }

    async fn introspect_table_unique_constraints_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<UniqueConstraint>> {
        self.introspect_table_unique_constraints(&OptionallyQualifiedName::quoted(
            table_name,
            schema_name,
        )?)
        .await
    }

    /// Introspects the foreign key constraints of a given table.
    async fn introspect_table_foreign_key_constraints(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Vec<ForeignKeyConstraint>> {
        let (schema_name, table_name) = self.introspected_names(table_name).await?;
        self.create_schema_provider()
            .await?
            .get_foreign_key_constraints_for_table(schema_name.as_deref(), &table_name)
            .await
    }

    async fn introspect_table_foreign_key_constraints_by_unquoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<ForeignKeyConstraint>> {
        self.introspect_table_foreign_key_constraints(&OptionallyQualifiedName::unquoted(
            table_name,
            schema_name,
        )?)
        .await
    }

    async fn introspect_table_foreign_key_constraints_by_quoted_name(
        &self,
        table_name: &str,
        schema_name: Option<&str>,
    ) -> R<Vec<ForeignKeyConstraint>> {
        self.introspect_table_foreign_key_constraints(&OptionallyQualifiedName::quoted(
            table_name,
            schema_name,
        )?)
        .await
    }

    /// The schema name and the unqualified name of `introspectTableObjects()`
    /// and `introspectTableOptions()`: normalized, the current schema when
    /// the name is unqualified.
    async fn introspected_names(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<(Option<String>, String)> {
        let folding = self.platform().get_unquoted_identifier_folding();
        let schema_name = match table_name.get_qualifier() {
            Some(qualifier) => Some(qualifier.to_normalized_value(folding)),
            None => self.get_current_schema_name().await?,
        };
        Ok((
            schema_name,
            table_name
                .get_unqualified_name()
                .to_normalized_value(folding),
        ))
    }

    /// `introspectTableOptions($tableName)` (private).
    async fn introspect_table_options(
        &self,
        table_name: &OptionallyQualifiedName,
    ) -> R<Option<PhpArray>> {
        let (schema_name, table_name) = self.introspected_names(table_name).await?;
        self.create_schema_provider()
            .await?
            .get_options_for_table(schema_name.as_deref(), &table_name)
            .await
    }

    /// Introspects the views in the current database.
    async fn introspect_views(&self) -> R<Vec<View>> {
        self.create_schema_provider().await?.get_all_views().await
    }

    /// Introspects the sequences in the current database.
    async fn introspect_sequences(&self) -> R<Vec<Sequence>> {
        let filter = self
            .connection()
            .get_configuration()
            .get_schema_assets_filter()
            .clone();
        Ok(self
            .create_schema_provider()
            .await?
            .get_all_sequences()
            .await?
            .into_iter()
            .filter(|sequence| filter(&sequence.get_name()))
            .collect())
    }

    /// `createSchemaProvider()` (private).
    async fn create_schema_provider(
        &self,
    ) -> R<IntrospectingSchemaProvider<Self::MetadataProvider>> {
        let current_schema_name = self.get_current_schema_name().await?;
        let configuration = self.create_schema_config().await?.to_table_configuration();
        Ok(IntrospectingSchemaProvider::new(
            self.create_metadata_provider().await?,
            current_schema_name.as_deref(),
            configuration,
        ))
    }

    // drop*() Methods

    /// Drops a database.
    async fn drop_database(&self, database: &str) -> R<()> {
        let sql = self.platform().get_drop_database_sql(database)?;
        self.execute_statement(&sql).await
    }

    /// Drops a schema.
    async fn drop_schema(&self, schema_name: &str) -> R<()> {
        let sql = self.platform().get_drop_schema_sql(schema_name)?;
        self.execute_statement(&sql).await
    }

    /// Drops the given table.
    async fn drop_table(&self, name: &str) -> R<()> {
        let sql = self.platform().get_drop_table_sql(name);
        self.execute_statement(&sql).await
    }

    /// Drops the index from the given table.
    async fn drop_index(&self, index: &str, table: &str) -> R<()> {
        let sql = self.platform().get_drop_index_sql(index, table);
        self.execute_statement(&sql).await
    }

    /// Drops a foreign key from a table.
    async fn drop_foreign_key(&self, name: &str, table: &str) -> R<()> {
        let sql = self.platform().get_drop_foreign_key_sql(name, table)?;
        self.execute_statement(&sql).await
    }

    /// Drops a sequence with a given name.
    async fn drop_sequence(&self, name: &str) -> R<()> {
        let sql = self.platform().get_drop_sequence_sql(name)?;
        self.execute_statement(&sql).await
    }

    /// Drops the unique constraint from the given table.
    async fn drop_unique_constraint(&self, name: &str, table_name: &str) -> R<()> {
        let sql = self
            .platform()
            .get_drop_unique_constraint_sql(name, table_name);
        self.execute_statement(&sql).await
    }

    /// Drops a view.
    async fn drop_view(&self, name: &str) -> R<()> {
        let sql = self.platform().get_drop_view_sql(name);
        self.execute_statement(&sql).await
    }

    // create*() Methods

    async fn create_schema_objects(&self, schema: &Schema) -> R<()> {
        self.execute_statements(&schema.to_sql(self.platform())?)
            .await
    }

    /// Creates a new database.
    async fn create_database(&self, database: &str) -> R<()> {
        let sql = self.platform().get_create_database_sql(database)?;
        self.execute_statement(&sql).await
    }

    /// Creates a new table.
    async fn create_table(&self, table: &Table) -> R<()> {
        self.execute_statements(&self.platform().get_create_table_sql(table)?)
            .await
    }

    /// Creates a new sequence.
    async fn create_sequence(&self, sequence: &Sequence) -> R<()> {
        let sql = self.platform().get_create_sequence_sql(sequence)?;
        self.execute_statement(&sql).await
    }

    /// Creates a new index on a table.
    async fn create_index(&self, index: &Index, table: &str) -> R<()> {
        let sql = self.platform().get_create_index_sql(index, table)?;
        self.execute_statement(&sql).await
    }

    /// Creates a new foreign key.
    async fn create_foreign_key(&self, foreign_key: &ForeignKeyConstraint, table: &str) -> R<()> {
        let sql = self
            .platform()
            .get_create_foreign_key_sql(foreign_key, table)?;
        self.execute_statement(&sql).await
    }

    /// Creates a unique constraint on a table.
    async fn create_unique_constraint(
        &self,
        unique_constraint: &UniqueConstraint,
        table_name: &str,
    ) -> R<()> {
        let sql = self
            .platform()
            .get_create_unique_constraint_sql(unique_constraint, table_name)?;
        self.execute_statement(&sql).await
    }

    /// Creates a new view.
    async fn create_view(&self, view: &View) -> R<()> {
        let sql = self
            .platform()
            .get_create_view_sql(&view.get_quoted_name(self.platform()), view.get_sql());
        self.execute_statement(&sql).await
    }

    async fn drop_schema_objects(&self, schema: &Schema) -> R<()> {
        self.execute_statements(&schema.to_drop_sql(self.platform())?)
            .await
    }

    /// Alters an existing schema.
    async fn alter_schema(&self, schema_diff: &SchemaDiff) -> R<()> {
        self.execute_statements(&self.platform().get_alter_schema_sql(schema_diff)?)
            .await
    }

    // alterTable() Methods

    /// Alters an existing tables schema.
    async fn alter_table(&self, table_diff: &TableDiff) -> R<()> {
        self.execute_statements(&self.platform().get_alter_table_sql(table_diff)?)
            .await
    }

    /// Renames a given table to another name.
    async fn rename_table(&self, name: &str, new_name: &str) -> R<()> {
        let sql = self.platform().get_rename_table_sql(name, new_name);
        self.execute_statement(&sql).await
    }

    // Methods for filtering return values of list*() methods to convert
    // the native DBMS data definition to a portable Doctrine definition

    /// `_getPortableDatabaseDefinition($database)`.
    fn get_portable_database_definition(&self, _database: &Row) -> R<String> {
        Err(NotSupported::new(&method("_getPortableDatabaseDefinition")).into())
    }

    /// `_getPortableSequenceDefinition($sequence)`.
    fn get_portable_sequence_definition(&self, _sequence: &Row) -> R<Sequence> {
        Err(NotSupported::new(&method("_getPortableSequenceDefinition")).into())
    }

    /// Independent of the database the keys of the column list result are
    /// lowercased. The name of the created column instance however is kept
    /// in its case.
    fn get_portable_table_column_list(
        &self,
        _table: &str,
        _database: &str,
        rows: &[Row],
    ) -> R<ArrayMap<Column>> {
        let mut list = ArrayMap::new();
        for row in rows {
            let column = self.get_portable_table_column_definition(row)?;

            let name = lower(&column.get_quoted_name(self.platform()));
            list.insert(name, column);
        }
        Ok(list)
    }

    /// Aggregates and groups the index results according to the required
    /// data result.
    fn get_portable_table_indexes_list(
        &self,
        rows: &[Row],
        _table_name: &str,
    ) -> R<ArrayMap<Index>> {
        portable_table_indexes_list(rows)
    }

    /// `_getPortableTableForeignKeysList($rows)`.
    async fn get_portable_table_foreign_keys_list(
        &self,
        rows: &[Row],
    ) -> R<Vec<ForeignKeyConstraint>> {
        rows.iter()
            .map(|row| self.get_portable_table_foreign_key_definition(row))
            .collect()
    }

    /// `executeStatements($sql)` (private).
    async fn execute_statements(&self, sql: &[String]) -> R<()> {
        for query in sql {
            self.execute_statement(query).await?;
        }
        Ok(())
    }

    /// `$this->connection->executeStatement($sql)`.
    async fn execute_statement(&self, sql: &str) -> R<()> {
        self.connection()
            .execute_statement(sql, &PhpArray::new(), &ParameterTypes::new())
            .await
            .map(|_| ())
    }

    /// Returns a [`Schema`] instance representing the current database
    /// schema.
    async fn introspect_schema(&self) -> R<Schema> {
        let schema_names = if self.platform().supports_schemas() {
            self.list_schema_names().await?
        } else {
            Vec::new()
        };

        let sequences = if self.platform().supports_sequences() {
            self.list_sequences().await?
        } else {
            Vec::new()
        };

        let tables = self.list_tables().await?;

        let schema_names: Vec<&str> = schema_names.iter().map(String::as_str).collect();
        Schema::new(
            tables,
            sequences,
            Some(self.create_schema_config().await?),
            &schema_names,
        )
    }

    /// Creates the configuration for this schema.
    async fn create_schema_config(&self) -> R<SchemaConfig> {
        let mut schema_config = SchemaConfig::new();
        schema_config.set_max_identifier_length(self.platform().get_max_identifier_length());
        schema_config.set_name(self.get_current_schema_name().await?.as_deref());

        let params = self.connection().get_params();
        let mut default_table_options = match params.get(&Key::from("defaultTableOptions")) {
            None | Some(Value::Null) => PhpArray::new(),
            Some(Value::Array(options)) => options.clone(),
            Some(other) => {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    format!(
                        "Doctrine\\DBAL\\Schema\\SchemaConfig::setDefaultTableOptions(): Argument #1 ($defaultTableOptions) must be of type array, {} given",
                        other.debug_type()
                    ),
                ));
            }
        };

        let has_charset = default_table_options
            .get(&Key::from("charset"))
            .is_some_and(|v| !v.is_null());
        if !has_charset
            && let Some(charset) = params.get(&Key::from("charset")).filter(|v| !v.is_null())
        {
            default_table_options.insert("charset", charset.clone());
        }

        schema_config.set_default_table_options(default_table_options);

        Ok(schema_config)
    }

    /// `getDatabase($methodName)` (private).
    async fn get_database(&self, method_name: &str) -> R<String> {
        match self.connection().get_database().await? {
            None | Some(Value::Null) => Err(Exception::database_required(&method(method_name))),
            Some(database) => crate::platforms::sqlite::text_value(Some(&database)),
        }
    }

    /// Groups the rows representing database object elements by table
    /// they belong to.
    fn group_by_table(&self, rows: Vec<Row>) -> R<ArrayMap<Vec<Row>>> {
        let mut data: ArrayMap<Vec<Row>> = ArrayMap::new();
        for row in rows {
            let table_name = self.get_portable_table_definition(&row)?;
            match data.get_mut(&table_name) {
                Some(rows) => rows.push(row),
                None => data.insert(table_name, vec![row]),
            }
        }
        Ok(data)
    }
}

/// `AbstractSchemaManager::_getPortableTableIndexesList()`, for a manager
/// that overrides it and calls `parent::`.
pub(crate) fn portable_table_indexes_list(rows: &[Row]) -> R<ArrayMap<Index>> {
    struct Data {
        name: String,
        columns: Vec<String>,
        unique: bool,
        primary: bool,
        flags: Vec<String>,
        where_: Option<Value>,
        lengths: Vec<Value>,
    }

    let mut result: ArrayMap<Data> = ArrayMap::new();
    for row in rows {
        let index_name = field_text(row, "key_name")?;
        let primary = field(row, "primary").is_some_and(Value::is_truthy);
        let key_name = lower(if primary { "primary" } else { &index_name });

        if !result.contains_key(&key_name) {
            let flags = match field(row, "flags") {
                Some(Value::Array(flags)) => flags
                    .values()
                    .map(|flag| crate::platforms::sqlite::text_value(Some(flag)))
                    .collect::<R<Vec<_>>>()?,
                _ => Vec::new(),
            };
            result.insert(
                key_name.clone(),
                Data {
                    name: index_name,
                    columns: Vec::new(),
                    unique: !field(row, "non_unique").is_some_and(Value::is_truthy),
                    primary,
                    flags,
                    where_: isset(row, "where")
                        .then(|| field(row, "where").cloned())
                        .flatten(),
                    lengths: Vec::new(),
                },
            );
        }

        if let Some(data) = result.get_mut(&key_name) {
            data.columns.push(field_text(row, "column_name")?);
            data.lengths
                .push(field(row, "length").cloned().unwrap_or(Value::Null));
        }
    }

    let mut indexes = ArrayMap::new();
    for (index_key, data) in result.iter() {
        let mut options = PhpArray::new();
        options.insert(
            "lengths",
            Value::Array(PhpArray::list(data.lengths.clone())),
        );
        if let Some(where_) = &data.where_ {
            options.insert("where", where_.clone());
        }
        indexes.insert(
            index_key,
            Index::new(
                Some(&data.name),
                data.columns.clone(),
                data.unique,
                data.primary,
                data.flags.clone(),
                options,
            )?,
        );
    }

    Ok(indexes)
}

/// `testTableName()`'s formatted name: `schema.table` or `table`.
fn formatted_name(table_name: &OptionallyQualifiedName) -> String {
    let mut formatted = table_name.get_unqualified_name().get_value().to_owned();
    if let Some(qualifier) = table_name.get_qualifier() {
        formatted = format!("{}.{formatted}", qualifier.get_value());
    }
    formatted
}
