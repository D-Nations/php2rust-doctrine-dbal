//! Port of Doctrine\DBAL\Schema\Comparator.php: compares two schemas
//! (or two tables) and returns a `SchemaDiff` (or a `TableDiff`).
//!
//! The platform comparators (`platforms::sqlite::Comparator`, ...) only
//! override `compareTables()` to normalize both tables first; here they
//! give the base comparator a table normalizer, so that a call through
//! the base (`compareSchemas()`) reaches the override as in PHP.
//!
//! PHP's keyed arrays are `ArrayMap`s; `foreach` iterates a copy of the
//! array, so the loops that `unset()` while iterating walk a snapshot.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Comparator.php

use std::fmt;

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;

use super::abstract_asset::AbstractAsset;
use super::array_map::ArrayMap;
use super::column::Column;
use super::column_diff::ColumnDiff;
use super::comparator_config::ComparatorConfig;
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::Index;
use super::schema::Schema;
use super::schema_diff::{SchemaDiff, SchemaDiffParts};
use super::sequence::Sequence;
use super::table::Table;
use super::table_diff::{TableDiff, TableDiffParts};

/// A platform comparator's `compareTables()` override: the table it
/// compares in place of the given one.
pub(crate) type TableNormalizer<'a> = Box<dyn Fn(&Table) -> Result<Table, Exception> + 'a>;

/// Compares two Schemas and return an instance of `SchemaDiff`.
pub struct Comparator<'a> {
    platform: &'a dyn AbstractPlatform,
    config: ComparatorConfig,
    normalizer: Option<TableNormalizer<'a>>,
}

impl fmt::Debug for Comparator<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Comparator")
            .field("platform", &self.platform.class())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// `strtolower()` (ASCII).
fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

impl<'a> Comparator<'a> {
    /// `@internal` The comparator can be only instantiated by a schema
    /// manager.
    pub fn new(platform: &'a dyn AbstractPlatform, config: ComparatorConfig) -> Self {
        Self {
            platform,
            config,
            normalizer: None,
        }
    }

    /// A platform comparator: `compareTables()` normalizes both tables
    /// with `normalizer` before comparing them.
    pub(crate) fn with_normalizer(
        platform: &'a dyn AbstractPlatform,
        config: ComparatorConfig,
        normalizer: TableNormalizer<'a>,
    ) -> Self {
        Self {
            platform,
            config,
            normalizer: Some(normalizer),
        }
    }

    /// Returns the differences between the schemas.
    pub fn compare_schemas(
        &self,
        old_schema: &Schema,
        new_schema: &Schema,
    ) -> Result<SchemaDiff, Exception> {
        let mut parts = SchemaDiffParts::default();

        // `$schema->getName()` without its deprecation (an internal call).
        let old_schema_name = AbstractAsset::get_name(old_schema);
        let new_schema_name = AbstractAsset::get_name(new_schema);

        for new_namespace in new_schema.get_namespaces() {
            if !old_schema.has_namespace(&new_namespace) {
                parts.created_schemas.push(new_namespace);
            }
        }

        for old_namespace in old_schema.get_namespaces() {
            if !new_schema.has_namespace(&old_namespace) {
                parts.dropped_schemas.push(old_namespace);
            }
        }

        for new_table in new_schema.get_tables() {
            let new_table_name = new_table.get_shortest_name(Some(&new_schema_name));
            if old_schema.has_table(&new_table_name) {
                let table_diff = self.compare_tables(
                    old_schema.get_table(&new_table_name)?,
                    new_schema.get_table(&new_table_name)?,
                )?;

                if !table_diff.is_empty() {
                    parts.altered_tables.push(table_diff);
                }
            } else {
                parts
                    .created_tables
                    .push(new_schema.get_table(&new_table_name)?.clone());
            }
        }

        // Check if there are tables removed
        for old_table in old_schema.get_tables() {
            let old_table_name = old_table.get_shortest_name(Some(&old_schema_name));

            let old_table = old_schema.get_table(&old_table_name)?;
            if !new_schema.has_table(&old_table_name) {
                parts.dropped_tables.push(old_table.clone());
            }
        }

        for new_sequence in new_schema.get_sequences() {
            let new_sequence_name = new_sequence.get_shortest_name(Some(&new_schema_name));
            if !old_schema.has_sequence(&new_sequence_name) {
                if !Self::is_auto_increment_sequence_in_schema(old_schema, new_sequence)? {
                    parts.created_sequences.push(new_sequence.clone());
                }
            } else if Self::diff_sequence_inner(
                new_sequence,
                old_schema.get_sequence(&new_sequence_name)?,
            ) {
                parts
                    .altered_sequences
                    .push(new_schema.get_sequence(&new_sequence_name)?.clone());
            }
        }

        for old_sequence in old_schema.get_sequences() {
            if Self::is_auto_increment_sequence_in_schema(new_schema, old_sequence)? {
                continue;
            }

            let old_sequence_name = old_sequence.get_shortest_name(Some(&old_schema_name));

            if !new_schema.has_sequence(&old_sequence_name) {
                parts.dropped_sequences.push(old_sequence.clone());
            }
        }

        Ok(SchemaDiff::new(parts))
    }

    fn is_auto_increment_sequence_in_schema(
        schema: &Schema,
        sequence: &Sequence,
    ) -> Result<bool, Exception> {
        for table in schema.get_tables() {
            if sequence.is_auto_increments_for_inner(table)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// `diffSequence($sequence1, $sequence2)`.
    pub fn diff_sequence(&self, sequence1: &Sequence, sequence2: &Sequence) -> bool {
        Self::diff_sequence_inner(sequence1, sequence2)
    }

    fn diff_sequence_inner(sequence1: &Sequence, sequence2: &Sequence) -> bool {
        if sequence1.get_allocation_size() != sequence2.get_allocation_size() {
            return true;
        }

        sequence1.get_initial_value() != sequence2.get_initial_value()
    }

    /// Compares the tables and returns the difference between them.
    pub fn compare_tables(
        &self,
        old_table: &Table,
        new_table: &Table,
    ) -> Result<TableDiff, Exception> {
        match &self.normalizer {
            Some(normalize) => {
                self.compare_tables_inner(&normalize(old_table)?, &normalize(new_table)?)
            }
            None => self.compare_tables_inner(old_table, new_table),
        }
    }

    #[allow(clippy::too_many_lines, reason = "one PHP method, kept in its order")]
    fn compare_tables_inner(
        &self,
        old_table: &Table,
        new_table: &Table,
    ) -> Result<TableDiff, Exception> {
        let should_report_modified_indexes = self.config.get_report_modified_indexes();
        if should_report_modified_indexes {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6890", || {
                "Detection of modified indexes is deprecated. Please disable it by configuring the comparator \
                 using ComparatorConfig::withReportModifiedIndexes(false)."
                    .to_owned()
            });
        }

        let mut added_columns: ArrayMap<Column> = ArrayMap::new();
        let mut modified_columns: ArrayMap<ColumnDiff> = ArrayMap::new();
        let mut dropped_columns: ArrayMap<Column> = ArrayMap::new();
        let mut added_indexes: ArrayMap<Index> = ArrayMap::new();
        let mut modified_indexes: Vec<Index> = Vec::new();
        let mut dropped_indexes: ArrayMap<Index> = ArrayMap::new();
        let mut renamed_indexes: ArrayMap<Index> = ArrayMap::new();
        let mut added_foreign_keys: ArrayMap<ForeignKeyConstraint> = ArrayMap::new();
        let mut dropped_foreign_keys: ArrayMap<ForeignKeyConstraint> = ArrayMap::new();

        // See if all the columns in the old table exist in the new table
        for new_column in new_table.get_columns() {
            let new_column_name = lower(&new_column.get_name());
            if !old_table.has_column(&new_column_name) {
                added_columns.insert(new_column_name, new_column.clone());
            }
        }

        // See if there are any removed columns in the new table
        for old_column in old_table.get_columns() {
            let old_column_name = lower(&old_column.get_name());

            // See if column is removed in the new table.
            if !new_table.has_column(&old_column_name) {
                dropped_columns.insert(old_column_name, old_column.clone());
                continue;
            }

            let new_column = new_table.get_column(&old_column_name)?;

            if !self.columns_equal(old_column, new_column)? {
                modified_columns.insert(
                    old_column_name,
                    ColumnDiff::new(old_column.clone(), new_column.clone()),
                );
            }
        }

        let renamed_column_names = new_table.get_renamed_columns();

        let added_snapshot: Vec<(String, Column)> = added_columns
            .iter()
            .map(|(k, v)| (k.to_owned(), v.clone()))
            .collect();
        for (added_column_name, added_column) in added_snapshot {
            let Some(removed_column_name) = renamed_column_names.get(&added_column.get_name())
            else {
                continue;
            };
            let removed_column_name = lower(removed_column_name);
            // Explicitly renamed columns need to be diffed, because their
            // types can also have changed
            let Some(dropped_column) = dropped_columns.get(&removed_column_name).cloned() else {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    "Doctrine\\DBAL\\Schema\\ColumnDiff::__construct(): Argument #1 ($oldColumn) must be of type Doctrine\\DBAL\\Schema\\Column, null given",
                ));
            };
            modified_columns.insert(
                removed_column_name.clone(),
                ColumnDiff::new(dropped_column, added_column),
            );

            added_columns.remove(&added_column_name);
            dropped_columns.remove(&removed_column_name);
        }

        if self.config.get_detect_renamed_columns() {
            self.detect_renamed_columns(
                &mut modified_columns,
                &mut added_columns,
                &mut dropped_columns,
            )?;
        }

        let old_primary_key = old_table.primary_key_index();
        let new_primary_key = new_table.primary_key_index();

        // See if all the indexes from the old table exist in the new one
        for new_index in new_table.get_indexes().values() {
            let new_index_name = new_index.get_name();

            if (!new_index.is_primary() || old_primary_key.is_none())
                && !old_table.has_index(&new_index_name)
            {
                added_indexes.push(new_index.clone());
            }
        }

        // See if there are any removed indexes in the new table
        for old_index in old_table.get_indexes().values() {
            let old_index_name = old_index.get_name();

            // See if the index is removed in the new table.
            if (old_index.is_primary() && new_primary_key.is_none())
                || !old_index.is_primary() && !new_table.has_index(&old_index_name)
            {
                dropped_indexes.push(old_index.clone());
                continue;
            }

            // See if index has changed in the new table.
            let new_index = match (old_index.is_primary(), new_primary_key) {
                (true, Some(primary_key)) => primary_key,
                _ => new_table.get_index(&old_index_name)?,
            };

            if !Self::diff_index(old_index, new_index) {
                continue;
            }

            if should_report_modified_indexes {
                modified_indexes.push(new_index.clone());
            } else {
                dropped_indexes.push(old_index.clone());
                added_indexes.push(new_index.clone());
            }
        }

        if self.config.get_detect_renamed_indexes() {
            renamed_indexes =
                Self::detect_renamed_indexes(&mut added_indexes, &mut dropped_indexes);
        }

        let mut old_foreign_keys: ArrayMap<&ForeignKeyConstraint> = old_table
            .get_foreign_keys()
            .iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();
        let mut new_foreign_keys: ArrayMap<&ForeignKeyConstraint> = new_table
            .get_foreign_keys()
            .iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect();

        let old_snapshot: Vec<(String, &ForeignKeyConstraint)> = old_foreign_keys
            .iter()
            .map(|(k, v)| (k.to_owned(), *v))
            .collect();
        for (old_key, old_foreign_key) in old_snapshot {
            let new_snapshot: Vec<(String, &ForeignKeyConstraint)> = new_foreign_keys
                .iter()
                .map(|(k, v)| (k.to_owned(), *v))
                .collect();
            for (new_key, new_foreign_key) in new_snapshot {
                if !Self::diff_foreign_key(old_foreign_key, new_foreign_key) {
                    old_foreign_keys.remove(&old_key);
                    new_foreign_keys.remove(&new_key);
                } else if lower(&old_foreign_key.get_name()) == lower(&new_foreign_key.get_name()) {
                    dropped_foreign_keys.insert(old_key.clone(), old_foreign_key.clone());
                    added_foreign_keys.insert(new_key.clone(), new_foreign_key.clone());

                    old_foreign_keys.remove(&old_key);
                    new_foreign_keys.remove(&new_key);
                }
            }
        }

        for old_foreign_key in old_foreign_keys.values() {
            dropped_foreign_keys.push((*old_foreign_key).clone());
        }

        for new_foreign_key in new_foreign_keys.values() {
            added_foreign_keys.push((*new_foreign_key).clone());
        }

        Ok(TableDiff::new(
            old_table.clone(),
            TableDiffParts {
                added_columns: added_columns.values().cloned().collect(),
                changed_columns: modified_columns.values().cloned().collect(),
                dropped_columns: dropped_columns.values().cloned().collect(),
                added_indexes: added_indexes.values().cloned().collect(),
                modified_indexes,
                dropped_indexes: dropped_indexes.values().cloned().collect(),
                renamed_indexes: renamed_indexes
                    .iter()
                    .map(|(k, v)| (k.to_owned(), v.clone()))
                    .collect(),
                added_foreign_keys: added_foreign_keys.values().cloned().collect(),
                modified_foreign_keys: Vec::new(),
                dropped_foreign_keys: dropped_foreign_keys.values().cloned().collect(),
            },
        )
        .with_dropped_foreign_key_keys(dropped_foreign_keys.keys().map(str::to_owned).collect()))
    }

    /// Try to find columns that only changed their name, rename operations
    /// maybe cheaper than add/drop however ambiguities between different
    /// possibilities should not lead to renaming at all.
    fn detect_renamed_columns(
        &self,
        modified_columns: &mut ArrayMap<ColumnDiff>,
        added_columns: &mut ArrayMap<Column>,
        removed_columns: &mut ArrayMap<Column>,
    ) -> Result<(), Exception> {
        let mut candidates_by_name: ArrayMap<Vec<(Column, Column)>> = ArrayMap::new();

        for (added_column_name, added_column) in added_columns.iter() {
            for removed_column in removed_columns.values() {
                if self.columns_equal(added_column, removed_column)? {
                    let pair = (removed_column.clone(), added_column.clone());
                    match candidates_by_name.get_mut(added_column_name) {
                        Some(candidates) => candidates.push(pair),
                        None => candidates_by_name.insert(added_column_name, vec![pair]),
                    }
                }
            }
        }

        for (added_column_name, candidates) in candidates_by_name.iter() {
            let [(old_column, new_column)] = candidates.as_slice() else {
                continue;
            };
            let old_column_name = lower(&old_column.get_name());

            if modified_columns.contains_key(&old_column_name) {
                continue;
            }

            modified_columns.insert(
                old_column_name.clone(),
                ColumnDiff::new(old_column.clone(), new_column.clone()),
            );

            added_columns.remove(added_column_name);
            removed_columns.remove(&old_column_name);
        }

        Ok(())
    }

    /// Try to find indexes that only changed their name, rename operations
    /// maybe cheaper than add/drop however ambiguities between different
    /// possibilities should not lead to renaming at all.
    fn detect_renamed_indexes(
        added_indexes: &mut ArrayMap<Index>,
        removed_indexes: &mut ArrayMap<Index>,
    ) -> ArrayMap<Index> {
        let mut candidates_by_name: ArrayMap<Vec<(String, String)>> = ArrayMap::new();
        let mut removed_index_match_count: ArrayMap<u64> = ArrayMap::new();

        // Gather possible rename candidates by comparing each added and
        // removed index based on semantics.
        for (added_index_key, added_index) in added_indexes.iter() {
            for (removed_index_key, removed_index) in removed_indexes.iter() {
                if Self::diff_index(added_index, removed_index) {
                    continue;
                }

                let pair = (removed_index_key.to_owned(), added_index_key.to_owned());
                let name = added_index.get_name();
                match candidates_by_name.get_mut(&name) {
                    Some(candidates) => candidates.push(pair),
                    None => candidates_by_name.insert(name, vec![pair]),
                }

                let count = removed_index_match_count
                    .get(removed_index_key)
                    .copied()
                    .unwrap_or(0);
                removed_index_match_count.insert(removed_index_key, count + 1);
            }
        }

        let mut renamed_indexes: ArrayMap<Index> = ArrayMap::new();

        for candidates in candidates_by_name.values() {
            // If the current rename candidate contains exactly one
            // semantically equal index, we can safely rename it.
            // Otherwise, it is unclear if a rename action is really
            // intended, therefore we let those ambiguous indexes be
            // added/dropped.
            let [(removed_index_key, added_index_key)] = candidates.as_slice() else {
                continue;
            };

            // Likewise, a removed index that matches more than one added
            // index is ambiguous.
            if removed_index_match_count
                .get(removed_index_key)
                .is_some_and(|count| *count > 1)
            {
                continue;
            }

            let (Some(removed_index), Some(added_index)) = (
                removed_indexes.get(removed_index_key),
                added_indexes.get(added_index_key),
            ) else {
                continue;
            };
            let removed_index_name = lower(&removed_index.get_name());

            renamed_indexes.insert(removed_index_name, added_index.clone());
            added_indexes.remove(added_index_key);
            removed_indexes.remove(removed_index_key);
        }

        renamed_indexes
    }

    /// `diffForeignKey($key1, $key2)` (protected).
    fn diff_foreign_key(key1: &ForeignKeyConstraint, key2: &ForeignKeyConstraint) -> bool {
        let lower_all =
            |columns: Vec<String>| -> Vec<String> { columns.iter().map(|c| lower(c)).collect() };

        if lower_all(key1.unquoted_local_columns()) != lower_all(key2.unquoted_local_columns()) {
            return true;
        }

        if lower_all(key1.unquoted_foreign_columns()) != lower_all(key2.unquoted_foreign_columns())
        {
            return true;
        }

        if key1.unqualified_foreign_table_name() != key2.unqualified_foreign_table_name() {
            return true;
        }

        if key1.on_event("onUpdate") != key2.on_event("onUpdate") {
            return true;
        }

        key1.on_event("onDelete") != key2.on_event("onDelete")
    }

    /// Compares the definitions of the given columns (protected).
    fn columns_equal(&self, column1: &Column, column2: &Column) -> Result<bool, Exception> {
        self.platform.columns_equal(column1, column2)
    }

    /// Finds the difference between the indexes `index1` and `index2`
    /// (protected): true if there are any differences.
    fn diff_index(index1: &Index, index2: &Index) -> bool {
        !(index1.is_fulfilled_by(index2) && index2.is_fulfilled_by(index1))
    }
}
