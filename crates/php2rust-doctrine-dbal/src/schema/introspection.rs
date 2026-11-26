//! Port of Doctrine\DBAL\Schema\Introspection: the schema provider that
//! builds schema objects from the rows of a [`MetadataProvider`], and the
//! metadata processors that combine the rows of one object.
//!
//! PHP groups rows in nested arrays keyed by schema, table and object id;
//! here the groups are kept in insertion order in vectors, which is the
//! order PHP's arrays iterate in.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Introspection/IntrospectingSchemaProvider.php
//! Upstream: doctrine/dbal src/Schema/Introspection/MetadataProcessor/ForeignKeyConstraintColumnMetadataProcessor.php
//! Upstream: doctrine/dbal src/Schema/Introspection/MetadataProcessor/IndexColumnMetadataProcessor.php
//! Upstream: doctrine/dbal src/Schema/Introspection/MetadataProcessor/PrimaryKeyConstraintColumnMetadataProcessor.php
//! Upstream: doctrine/dbal src/Schema/Introspection/MetadataProcessor/SequenceMetadataProcessor.php
//! Upstream: doctrine/dbal src/Schema/Introspection/MetadataProcessor/UniqueConstraintColumnMetadataProcessor.php
//! Upstream: doctrine/dbal src/Schema/Introspection/MetadataProcessor/ViewMetadataProcessor.php

use php_value::PhpArray;

use crate::deprecations;
use crate::exception::Exception;

use super::column::Column;
use super::foreign_key_constraint::{Deferrability, ForeignKeyConstraint};
use super::foreign_key_constraint_editor::ForeignKeyConstraintEditor;
use super::index::Index;
use super::index_editor::IndexEditor;
use super::metadata::{
    ForeignKeyConstraintColumnMetadataRow, IndexColumnMetadataRow, MetadataProvider,
    PrimaryKeyConstraintColumnRow, RowId, SequenceMetadataRow, UniqueConstraintColumnMetadataRow,
    ViewMetadataRow,
};
use super::name::{OptionallyQualifiedName, UnqualifiedName};
use super::primary_key_constraint::{PrimaryKeyConstraint, PrimaryKeyConstraintEditor};
use super::schema_config::TableConfiguration;
use super::sequence::Sequence;
use super::table::Table;
use super::unique_constraint::UniqueConstraint;
use super::unique_constraint_editor::UniqueConstraintEditor;
use super::view::View;

const METADATA_PROVIDER: &str = "Doctrine\\DBAL\\Schema\\Metadata\\MetadataProvider";
const LINK_UNIQUE_CONSTRAINTS: &str = "https://github.com/doctrine/dbal/pull/7461";

/// The value representing the `NULL` schema key in results grouped by
/// schema name.
const NULL_SCHEMA_KEY: &str = "";

/// An insertion-ordered map, as a PHP array iterates.
#[derive(Debug)]
struct Ordered<K, V>(Vec<(K, V)>);

impl<K: PartialEq, V> Ordered<K, V> {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn entry(&mut self, key: K, init: impl FnOnce() -> V) -> &mut V {
        let position = if let Some(position) = self.0.iter().position(|(k, _)| *k == key) {
            position
        } else {
            self.0.push((key, init()));
            self.0.len() - 1
        };
        &mut self.0[position].1
    }

    fn insert(&mut self, key: K, value: V) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some((_, slot)) => *slot = value,
            None => self.0.push((key, value)),
        }
    }
}

/// Rows grouped by schema and table.
type ByTable<T> = Ordered<String, Ordered<String, T>>;

/// Combines multiple [`ForeignKeyConstraintColumnMetadataRow`]s into a
/// [`ForeignKeyConstraint`].
#[derive(Debug, Clone)]
pub struct ForeignKeyConstraintColumnMetadataProcessor {
    current_schema_name: Option<String>,
}

impl ForeignKeyConstraintColumnMetadataProcessor {
    pub fn new(current_schema_name: Option<&str>) -> Self {
        Self {
            current_schema_name: current_schema_name.map(str::to_owned),
        }
    }

    pub fn initialize_editor(
        &self,
        row: &ForeignKeyConstraintColumnMetadataRow,
    ) -> Result<ForeignKeyConstraintEditor, Exception> {
        let mut editor = ForeignKeyConstraint::editor();

        if let Some(constraint_name) = row.get_name() {
            editor = editor.set_name(Some(UnqualifiedName::quoted(constraint_name)?));
        }

        let mut referenced_schema_name = row.get_referenced_schema_name();
        if referenced_schema_name == self.current_schema_name.as_deref() {
            referenced_schema_name = None;
        }

        editor = editor
            .set_referenced_table_name(OptionallyQualifiedName::quoted(
                row.get_referenced_table_name(),
                referenced_schema_name,
            )?)
            .set_match_type(row.get_match_type())
            .set_on_update_action(row.get_on_update_action())
            .set_on_delete_action(row.get_on_delete_action());

        if row.is_deferred() {
            editor = editor.set_deferrability(Deferrability::Deferred);
        } else if row.is_deferrable() {
            editor = editor.set_deferrability(Deferrability::Deferrable);
        }

        Ok(editor)
    }

    pub fn apply_row(
        &self,
        editor: ForeignKeyConstraintEditor,
        row: &ForeignKeyConstraintColumnMetadataRow,
    ) -> ForeignKeyConstraintEditor {
        editor
            .add_quoted_referencing_column_name(row.get_referencing_column_name())
            .add_quoted_referenced_column_name(row.get_referenced_column_name())
    }
}

/// Combines multiple [`IndexColumnMetadataRow`]s into an [`Index`].
#[derive(Debug, Clone, Copy, Default)]
pub struct IndexColumnMetadataProcessor;

impl IndexColumnMetadataProcessor {
    pub fn initialize_editor(
        &self,
        row: &IndexColumnMetadataRow,
    ) -> Result<IndexEditor, Exception> {
        Ok(Index::editor()
            .set_name(Some(UnqualifiedName::quoted(row.get_index_name())?))
            .set_type(row.get_type())
            .set_is_clustered(row.is_clustered())
            .set_predicate(row.get_predicate()))
    }

    pub fn apply_row(&self, editor: IndexEditor, row: &IndexColumnMetadataRow) -> IndexEditor {
        editor.add_quoted_column_name(row.get_column_name(), row.get_column_length())
    }
}

/// Combines multiple [`PrimaryKeyConstraintColumnRow`]s into a
/// [`PrimaryKeyConstraint`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PrimaryKeyConstraintColumnMetadataProcessor;

impl PrimaryKeyConstraintColumnMetadataProcessor {
    pub fn initialize_editor(
        &self,
        row: &PrimaryKeyConstraintColumnRow,
    ) -> PrimaryKeyConstraintEditor {
        // Ignore the constraint name in 4.x, since it represents primary key
        // constraints as indexes, and the name of the constraint may conflict
        // with the name of its backing index.
        PrimaryKeyConstraint::editor().set_is_clustered(row.is_clustered())
    }

    pub fn apply_row(
        &self,
        editor: PrimaryKeyConstraintEditor,
        row: &PrimaryKeyConstraintColumnRow,
    ) -> PrimaryKeyConstraintEditor {
        editor.add_quoted_column_name(row.get_column_name())
    }
}

/// Converts a [`SequenceMetadataRow`] into a [`Sequence`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceMetadataProcessor;

impl SequenceMetadataProcessor {
    pub fn create_object(&self, row: &SequenceMetadataRow) -> Result<Sequence, Exception> {
        Sequence::editor()
            .set_quoted_name(row.get_sequence_name(), row.get_schema_name())
            .set_allocation_size(row.get_allocation_size())
            .set_initial_value(row.get_initial_value())
            .set_cache_size(row.get_cache_size())
            .create()
    }
}

/// Combines multiple [`UniqueConstraintColumnMetadataRow`]s into a
/// [`UniqueConstraint`].
#[derive(Debug, Clone, Copy, Default)]
pub struct UniqueConstraintColumnMetadataProcessor;

impl UniqueConstraintColumnMetadataProcessor {
    pub fn initialize_editor(
        &self,
        row: &UniqueConstraintColumnMetadataRow,
    ) -> UniqueConstraintEditor {
        let mut editor = UniqueConstraint::editor();
        if let Some(name) = row.get_name() {
            editor = editor.set_quoted_name(name);
        }
        editor
    }

    pub fn apply_row(
        &self,
        editor: UniqueConstraintEditor,
        row: &UniqueConstraintColumnMetadataRow,
    ) -> UniqueConstraintEditor {
        editor.add_quoted_column_name(row.get_column_name())
    }
}

/// Converts a [`ViewMetadataRow`] into a [`View`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ViewMetadataProcessor;

impl ViewMetadataProcessor {
    pub fn create_object(&self, row: &ViewMetadataRow) -> Result<View, Exception> {
        View::editor()
            .set_quoted_name(row.get_view_name(), row.get_schema_name())
            .set_sql(row.get_definition())
            .create()
    }
}

/// Provides access to the database schema obtained by introspection.
///
/// If the underlying database platform supports schemas, and the object
/// is located in the current schema, the schema name will be omitted to
/// match the behavior of the platforms that do not support schemas.
///
/// `@internal` Should be used only by `AbstractSchemaManager`.
#[derive(Debug)]
pub struct IntrospectingSchemaProvider<M> {
    metadata_provider: M,
    current_schema_name: Option<String>,
    table_configuration: TableConfiguration,
}

impl<M: MetadataProvider> IntrospectingSchemaProvider<M> {
    pub fn new(
        metadata_provider: M,
        current_schema_name: Option<&str>,
        table_configuration: TableConfiguration,
    ) -> Self {
        Self {
            metadata_provider,
            current_schema_name: current_schema_name.map(str::to_owned),
            table_configuration,
        }
    }

    pub async fn get_all_database_names(&self) -> Result<Vec<UnqualifiedName>, Exception> {
        let mut database_names = Vec::new();
        for row in self.metadata_provider.get_all_database_names().await? {
            database_names.push(UnqualifiedName::quoted(row.get_database_name())?);
        }
        Ok(database_names)
    }

    pub async fn get_all_schema_names(&self) -> Result<Vec<UnqualifiedName>, Exception> {
        let mut schema_names = Vec::new();
        for row in self.metadata_provider.get_all_schema_names().await? {
            schema_names.push(UnqualifiedName::quoted(row.get_schema_name())?);
        }
        Ok(schema_names)
    }

    pub async fn get_all_tables(&self) -> Result<Vec<Table>, Exception> {
        let table_columns_by_table = self.get_columns_for_all_tables().await?;
        let indexes_by_table = self.get_indexes_for_all_tables().await?;
        let primary_key_constraints_by_table =
            self.get_primary_key_constraints_for_all_tables().await?;
        let unique_constraints_by_table = self.get_unique_constraints_for_all_tables().await?;
        let foreign_key_constraints_by_table =
            self.get_foreign_key_constraints_for_all_tables().await?;
        let table_options_by_table = self.get_options_for_all_tables().await?;

        let mut tables = Vec::new();

        for (schema_name_key, schema_tables) in table_columns_by_table.0 {
            let schema_name = if schema_name_key != NULL_SCHEMA_KEY
                && Some(schema_name_key.as_str()) != self.current_schema_name.as_deref()
            {
                Some(schema_name_key.as_str())
            } else {
                None
            };

            for (unqualified_name, table_columns) in schema_tables.0 {
                fn lookup<T: Clone>(
                    by_table: &ByTable<Vec<T>>,
                    schema_name_key: &String,
                    unqualified_name: &String,
                ) -> Option<Vec<T>> {
                    by_table
                        .get(schema_name_key)
                        .and_then(|tables| tables.get(unqualified_name))
                        .cloned()
                }

                let mut editor = Table::editor()
                    .set_name(OptionallyQualifiedName::quoted(
                        &unqualified_name,
                        schema_name,
                    )?)
                    .set_columns(table_columns)
                    .set_indexes(
                        lookup(&indexes_by_table, &schema_name_key, &unqualified_name)
                            .unwrap_or_default(),
                    );

                if let Some(constraint) = primary_key_constraints_by_table
                    .get(&schema_name_key)
                    .and_then(|tables| tables.get(&unqualified_name))
                {
                    editor = editor.set_primary_key_constraint(Some(constraint.clone()));
                }

                if let Some(constraints) = lookup(
                    &unique_constraints_by_table,
                    &schema_name_key,
                    &unqualified_name,
                ) {
                    editor = editor.set_unique_constraints(constraints);
                }

                if let Some(constraints) = lookup(
                    &foreign_key_constraints_by_table,
                    &schema_name_key,
                    &unqualified_name,
                ) {
                    editor = editor.set_foreign_key_constraints(constraints);
                }

                if let Some(options) = table_options_by_table
                    .get(&schema_name_key)
                    .and_then(|tables| tables.get(&unqualified_name))
                {
                    editor = editor.set_options(options.clone());
                }

                tables.push(
                    editor
                        .set_configuration(self.table_configuration)
                        .create()?,
                );
            }
        }

        Ok(tables)
    }

    pub async fn get_all_table_names(&self) -> Result<Vec<OptionallyQualifiedName>, Exception> {
        let mut table_names = Vec::new();
        for row in self.metadata_provider.get_all_table_names().await? {
            let mut schema_name = row.get_schema_name();
            let table_name = row.get_table_name();
            if schema_name == self.current_schema_name.as_deref() {
                schema_name = None;
            }
            table_names.push(OptionallyQualifiedName::quoted(table_name, schema_name)?);
        }
        Ok(table_names)
    }

    pub async fn get_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<Column>, Exception> {
        Ok(self
            .metadata_provider
            .get_table_columns_for_table(schema_name, table_name)
            .await?
            .into_iter()
            .map(super::metadata::TableColumnMetadataRow::into_column)
            .collect())
    }

    /// Returns columns of all tables, grouped by schema and table.
    async fn get_columns_for_all_tables(&self) -> Result<ByTable<Vec<Column>>, Exception> {
        let mut columns: ByTable<Vec<Column>> = Ordered::new();
        for row in self
            .metadata_provider
            .get_table_columns_for_all_tables()
            .await?
        {
            let schema_name = row.get_schema_name().unwrap_or(NULL_SCHEMA_KEY).to_owned();
            let table_name = row.get_table_name().to_owned();
            columns
                .entry(schema_name, Ordered::new)
                .entry(table_name, Vec::new)
                .push(row.into_column());
        }
        Ok(columns)
    }

    pub async fn get_indexes_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<Index>, Exception> {
        let processor = IndexColumnMetadataProcessor;
        let mut editors: Ordered<String, IndexEditor> = Ordered::new();
        for row in self
            .metadata_provider
            .get_index_columns_for_table(schema_name, table_name)
            .await?
        {
            let index_name = row.get_index_name().to_owned();
            if editors.get(&index_name).is_none() {
                editors.insert(index_name.clone(), processor.initialize_editor(&row)?);
            }
            apply(&mut editors, &index_name, |editor| {
                processor.apply_row(editor, &row)
            });
        }
        editors.0.into_iter().map(|(_, e)| e.create()).collect()
    }

    /// Returns indexes for all tables, grouped by schema and table.
    async fn get_indexes_for_all_tables(&self) -> Result<ByTable<Vec<Index>>, Exception> {
        let processor = IndexColumnMetadataProcessor;
        group_by_table(
            |row: &IndexColumnMetadataRow| RowId::String(row.get_index_name().to_owned()),
            |row| processor.initialize_editor(row),
            |editor, row| processor.apply_row(editor, row),
            IndexEditor::create,
            self.metadata_provider
                .get_index_columns_for_all_tables()
                .await?,
            |row| (row.get_schema_name(), row.get_table_name()),
        )
    }

    pub async fn get_primary_key_constraint_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Result<Option<PrimaryKeyConstraint>, Exception> {
        let processor = PrimaryKeyConstraintColumnMetadataProcessor;
        let mut editor: Option<PrimaryKeyConstraintEditor> = None;
        for row in self
            .metadata_provider
            .get_primary_key_constraint_columns_for_table(schema_name, table_name)
            .await?
        {
            let current = editor
                .take()
                .unwrap_or_else(|| processor.initialize_editor(&row));
            editor = Some(processor.apply_row(current, &row));
        }
        editor.map(PrimaryKeyConstraintEditor::create).transpose()
    }

    /// Returns the primary key constraints for all tables, grouped by
    /// schema and table.
    async fn get_primary_key_constraints_for_all_tables(
        &self,
    ) -> Result<ByTable<PrimaryKeyConstraint>, Exception> {
        let processor = PrimaryKeyConstraintColumnMetadataProcessor;
        let mut editors: ByTable<Option<PrimaryKeyConstraintEditor>> = Ordered::new();
        for row in self
            .metadata_provider
            .get_primary_key_constraint_columns_for_all_tables()
            .await?
        {
            let schema_name = row.get_schema_name().unwrap_or(NULL_SCHEMA_KEY).to_owned();
            let table_name = row.get_table_name().to_owned();
            let slot = editors
                .entry(schema_name, Ordered::new)
                .entry(table_name, || None);
            let current = slot
                .take()
                .unwrap_or_else(|| processor.initialize_editor(&row));
            *slot = Some(processor.apply_row(current, &row));
        }
        let mut constraints: ByTable<PrimaryKeyConstraint> = Ordered::new();
        for (schema_name, tables) in editors.0 {
            let mut by_table = Ordered::new();
            for (table_name, editor) in tables.0 {
                if let Some(editor) = editor {
                    by_table.0.push((table_name, editor.create()?));
                }
            }
            constraints.0.push((schema_name, by_table));
        }
        Ok(constraints)
    }

    pub async fn get_unique_constraints_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<UniqueConstraint>, Exception> {
        let Some(rows) = self
            .metadata_provider
            .get_unique_constraint_columns_for_table(schema_name, table_name)
            .await
        else {
            self.unique_constraints_not_implemented("getUniqueConstraintColumnsForTable");
            return Ok(Vec::new());
        };

        let processor = UniqueConstraintColumnMetadataProcessor;
        let mut editors: Ordered<RowId, UniqueConstraintEditor> = Ordered::new();
        for row in rows? {
            let id = row.get_id().clone();
            if editors.get(&id).is_none() {
                editors.insert(id.clone(), processor.initialize_editor(&row));
            }
            apply(&mut editors, &id, |editor| {
                processor.apply_row(editor, &row)
            });
        }
        editors.0.into_iter().map(|(_, e)| e.create()).collect()
    }

    fn unique_constraints_not_implemented(&self, method: &str) {
        let class = self.metadata_provider.class().to_owned();
        let method = method.to_owned();
        deprecations::trigger(LINK_UNIQUE_CONSTRAINTS, move || {
            format!("Not implementing {METADATA_PROVIDER}::{method}() in {class} is deprecated.")
        });
    }

    /// Returns the unique constraints, grouped by schema and table.
    async fn get_unique_constraints_for_all_tables(
        &self,
    ) -> Result<ByTable<Vec<UniqueConstraint>>, Exception> {
        let Some(rows) = self
            .metadata_provider
            .get_unique_constraint_columns_for_all_tables()
            .await
        else {
            self.unique_constraints_not_implemented("getUniqueConstraintColumnsForAllTables");
            return Ok(Ordered::new());
        };

        let processor = UniqueConstraintColumnMetadataProcessor;
        group_by_table(
            |row: &UniqueConstraintColumnMetadataRow| row.get_id().clone(),
            |row| Ok(processor.initialize_editor(row)),
            |editor, row| processor.apply_row(editor, row),
            UniqueConstraintEditor::create,
            rows?,
            |row| (row.get_schema_name(), row.get_table_name()),
        )
    }

    pub async fn get_foreign_key_constraints_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Result<Vec<ForeignKeyConstraint>, Exception> {
        let processor =
            ForeignKeyConstraintColumnMetadataProcessor::new(self.current_schema_name.as_deref());
        let mut editors: Ordered<RowId, ForeignKeyConstraintEditor> = Ordered::new();
        for row in self
            .metadata_provider
            .get_foreign_key_constraint_columns_for_table(schema_name, table_name)
            .await?
        {
            let id = row.get_id().clone();
            if editors.get(&id).is_none() {
                editors.insert(id.clone(), processor.initialize_editor(&row)?);
            }
            apply(&mut editors, &id, |editor| {
                processor.apply_row(editor, &row)
            });
        }
        editors.0.into_iter().map(|(_, e)| e.create()).collect()
    }

    /// Returns the foreign key constraints, grouped by schema and table.
    async fn get_foreign_key_constraints_for_all_tables(
        &self,
    ) -> Result<ByTable<Vec<ForeignKeyConstraint>>, Exception> {
        let processor =
            ForeignKeyConstraintColumnMetadataProcessor::new(self.current_schema_name.as_deref());
        group_by_table(
            |row: &ForeignKeyConstraintColumnMetadataRow| row.get_id().clone(),
            |row| processor.initialize_editor(row),
            |editor, row| processor.apply_row(editor, row),
            ForeignKeyConstraintEditor::create,
            self.metadata_provider
                .get_foreign_key_constraint_columns_for_all_tables()
                .await?,
            |row| (row.get_schema_name(), row.get_table_name()),
        )
    }

    pub async fn get_options_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Result<Option<PhpArray>, Exception> {
        Ok(self
            .metadata_provider
            .get_table_options_for_table(schema_name, table_name)
            .await?
            .into_iter()
            .next()
            .map(|row| row.get_options().clone()))
    }

    /// Returns options for all tables, grouped by schema and table.
    async fn get_options_for_all_tables(&self) -> Result<ByTable<PhpArray>, Exception> {
        let mut options: ByTable<PhpArray> = Ordered::new();
        for row in self
            .metadata_provider
            .get_table_options_for_all_tables()
            .await?
        {
            let schema_name = row.get_schema_name().unwrap_or(NULL_SCHEMA_KEY).to_owned();
            let table_name = row.get_table_name().to_owned();
            options
                .entry(schema_name, Ordered::new)
                .insert(table_name, row.get_options().clone());
        }
        Ok(options)
    }

    pub async fn get_all_views(&self) -> Result<Vec<View>, Exception> {
        let processor = ViewMetadataProcessor;
        let mut views = Vec::new();
        for row in self.metadata_provider.get_all_views().await? {
            views.push(processor.create_object(&row)?);
        }
        Ok(views)
    }

    pub async fn get_all_sequences(&self) -> Result<Vec<Sequence>, Exception> {
        let processor = SequenceMetadataProcessor;
        let mut sequences = Vec::new();
        for row in self.metadata_provider.get_all_sequences().await? {
            sequences.push(processor.create_object(&row)?);
        }
        Ok(sequences)
    }
}

/// `$processor->applyRow($editors[$key], $row)` on a builder-style editor.
fn apply<K: PartialEq, E>(editors: &mut Ordered<K, E>, key: &K, f: impl FnOnce(E) -> E) {
    if let Some(position) = editors.0.iter().position(|(k, _)| k == key) {
        let (k, editor) = editors.0.remove(position);
        editors.0.insert(position, (k, f(editor)));
    }
}

/// Groups rows by schema and table, builds one editor per grouping key,
/// and creates the objects.
fn group_by_table<R, E, T>(
    get_key: impl Fn(&R) -> RowId,
    initialize_editor: impl Fn(&R) -> Result<E, Exception>,
    apply_row: impl Fn(E, &R) -> E,
    create: impl Fn(E) -> Result<T, Exception>,
    rows: Vec<R>,
    table_of: impl Fn(&R) -> (Option<&str>, &str),
) -> Result<ByTable<Vec<T>>, Exception> {
    let mut editors: ByTable<Ordered<RowId, E>> = Ordered::new();
    for row in rows {
        let (schema_name, table_name) = table_of(&row);
        let schema_name = schema_name.unwrap_or(NULL_SCHEMA_KEY).to_owned();
        let table_name = table_name.to_owned();
        let key = get_key(&row);
        let table = editors
            .entry(schema_name, Ordered::new)
            .entry(table_name, Ordered::new);
        if table.get(&key).is_none() {
            table.0.push((key.clone(), initialize_editor(&row)?));
        }
        apply(table, &key, |editor| apply_row(editor, &row));
    }

    let mut result: ByTable<Vec<T>> = Ordered::new();
    for (schema_name, tables) in editors.0 {
        let mut by_table = Ordered::new();
        for (table_name, editors) in tables.0 {
            let objects = editors
                .0
                .into_iter()
                .map(|(_, editor)| create(editor))
                .collect::<Result<Vec<_>, _>>()?;
            by_table.0.push((table_name, objects));
        }
        result.0.push((schema_name, by_table));
    }
    Ok(result)
}
