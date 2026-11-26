//! Port of Doctrine\DBAL\Schema\SchemaDiff.php: the differences between
//! two schemas.
//!
//! `new SchemaDiff(...)` takes its eight arrays as a [`SchemaDiffParts`].
//! The altered tables keep PHP's `array_filter()` order; their keys are
//! not kept (PHP's getters hand out the filtered array, whose gaps no
//! caller reads).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SchemaDiff.php

use super::sequence::Sequence;
use super::table::Table;
use super::table_diff::TableDiff;

/// The arguments of `new SchemaDiff()`.
#[derive(Debug, Clone, Default)]
pub struct SchemaDiffParts {
    pub created_schemas: Vec<String>,
    pub dropped_schemas: Vec<String>,
    pub created_tables: Vec<Table>,
    pub altered_tables: Vec<TableDiff>,
    pub dropped_tables: Vec<Table>,
    pub created_sequences: Vec<Sequence>,
    pub altered_sequences: Vec<Sequence>,
    pub dropped_sequences: Vec<Sequence>,
}

/// Differences between two schemas.
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    parts: SchemaDiffParts,
}

impl SchemaDiff {
    /// `@internal` The diff can be only instantiated by a `Comparator`.
    pub fn new(mut parts: SchemaDiffParts) -> Self {
        parts.altered_tables.retain(|diff| !diff.is_empty());
        Self { parts }
    }

    pub fn get_created_schemas(&self) -> &[String] {
        &self.parts.created_schemas
    }

    pub fn get_dropped_schemas(&self) -> &[String] {
        &self.parts.dropped_schemas
    }

    pub fn get_created_tables(&self) -> &[Table] {
        &self.parts.created_tables
    }

    pub fn get_altered_tables(&self) -> &[TableDiff] {
        &self.parts.altered_tables
    }

    pub fn get_dropped_tables(&self) -> &[Table] {
        &self.parts.dropped_tables
    }

    pub fn get_created_sequences(&self) -> &[Sequence] {
        &self.parts.created_sequences
    }

    pub fn get_altered_sequences(&self) -> &[Sequence] {
        &self.parts.altered_sequences
    }

    pub fn get_dropped_sequences(&self) -> &[Sequence] {
        &self.parts.dropped_sequences
    }

    /// Returns whether the diff is empty (contains no changes).
    pub fn is_empty(&self) -> bool {
        let p = &self.parts;
        p.created_schemas.is_empty()
            && p.dropped_schemas.is_empty()
            && p.created_tables.is_empty()
            && p.altered_tables.is_empty()
            && p.dropped_tables.is_empty()
            && p.created_sequences.is_empty()
            && p.altered_sequences.is_empty()
            && p.dropped_sequences.is_empty()
    }
}
