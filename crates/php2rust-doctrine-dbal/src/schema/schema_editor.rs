//! Port of Doctrine\DBAL\Schema\SchemaEditor.php.
//!
//! A by-value builder. An exception PHP throws from a method is kept and
//! returned by `create()`; the calls after it change nothing (in PHP the
//! exception ends the chain). `modifyTable()` takes a function from the
//! table's editor to the changed editor; the variadic `setTables()` and
//! `setSequences()` take a `Vec`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SchemaEditor.php

use crate::exception::Exception;

use super::array_map::ArrayMap;
use super::exception;
use super::name::{Identifier, OptionallyQualifiedName, UnqualifiedName};
use super::schema::Schema;
use super::schema_config::SchemaConfig;
use super::sequence::Sequence;
use super::table::Table;
use super::table_editor::TableEditor;

#[derive(Debug, Clone, Default)]
pub struct SchemaEditor {
    error: Option<Exception>,
    default_namespace_name: Option<String>,
    tables: ArrayMap<Table>,
    sequences: ArrayMap<Sequence>,
}

impl SchemaEditor {
    /// `@internal` Use `Schema::editor()` or `Schema::edit()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs `f` unless an earlier call failed, keeping its error.
    fn apply(mut self, f: impl FnOnce(&mut Self) -> Result<(), Exception>) -> Self {
        if self.error.is_none()
            && let Err(e) = f(&mut self)
        {
            self.error = Some(e);
        }
        self
    }

    #[must_use]
    pub fn set_default_namespace(self, name: Option<&str>) -> Self {
        self.apply(|editor| {
            if !editor.tables.is_empty() || !editor.sequences.is_empty() {
                return Err(exception::default_namespace_can_only_be_set_on_empty_editor());
            }
            editor.default_namespace_name = name.map(str::to_owned);
            Ok(())
        })
    }

    #[must_use]
    pub fn set_tables(self, tables: Vec<Table>) -> Self {
        self.apply(|editor| {
            editor.tables.clear();
            for table in tables {
                editor.add_table_inner(table)?;
            }
            Ok(())
        })
    }

    #[must_use]
    pub fn add_table(self, table: Table) -> Self {
        self.apply(|editor| editor.add_table_inner(table))
    }

    fn add_table_inner(&mut self, table: Table) -> Result<(), Exception> {
        let name = table.get_object_name()?;
        let key = self.get_key(&name)?;

        if self.tables.contains_key(&key) {
            return Err(exception::schema_table_already_exists(&name));
        }

        self.tables.insert(key, table);

        Ok(())
    }

    #[must_use]
    pub fn modify_table(
        self,
        table_name: &OptionallyQualifiedName,
        modification: impl FnOnce(TableEditor) -> TableEditor,
    ) -> Self {
        self.apply(|editor| editor.modify_table_inner(table_name, modification))
    }

    fn modify_table_inner(
        &mut self,
        table_name: &OptionallyQualifiedName,
        modification: impl FnOnce(TableEditor) -> TableEditor,
    ) -> Result<(), Exception> {
        let key = self.get_key(table_name)?;

        let Some(table) = self.tables.get(&key) else {
            return Err(exception::schema_table_does_not_exist(table_name));
        };

        let new_table = modification(table.edit()?).create()?;

        let new_name = new_table.get_object_name()?;
        let old_resolved = self.resolve_name(table_name)?;
        let new_resolved = self.resolve_name(&new_name)?;

        if !qualifiers_equal(old_resolved.get_qualifier(), new_resolved.get_qualifier()) {
            return Err(exception::cannot_change_table_qualifier(
                &old_resolved,
                &new_resolved,
            ));
        }

        let new_key = self.get_key(&new_name)?;

        if new_key == key {
            self.tables.insert(key, new_table);

            return Ok(());
        }

        if self.tables.contains_key(&new_key) {
            return Err(exception::schema_table_already_exists(&new_name));
        }

        self.tables.remove(&key);
        self.tables.insert(new_key, new_table);

        Ok(())
    }

    #[must_use]
    pub fn modify_table_by_unquoted_name(
        self,
        table_name: &str,
        modification: impl FnOnce(TableEditor) -> TableEditor,
        qualifier: Option<&str>,
    ) -> Self {
        self.apply(|editor| {
            let name = OptionallyQualifiedName::unquoted(table_name, qualifier)?;
            editor.modify_table_inner(&name, modification)
        })
    }

    #[must_use]
    pub fn rename_table(
        self,
        old_table_name: &OptionallyQualifiedName,
        new_table_name: &UnqualifiedName,
    ) -> Self {
        self.apply(|editor| editor.rename_table_inner(old_table_name, new_table_name))
    }

    fn rename_table_inner(
        &mut self,
        old_table_name: &OptionallyQualifiedName,
        new_table_name: &UnqualifiedName,
    ) -> Result<(), Exception> {
        let new_name = OptionallyQualifiedName::new(
            new_table_name.get_identifier().clone(),
            old_table_name.get_qualifier().cloned(),
        );
        self.modify_table_inner(old_table_name, |editor| editor.set_name(new_name))
    }

    #[must_use]
    pub fn rename_table_by_unquoted_name(
        self,
        old_table_name: &str,
        new_table_name: &str,
        qualifier: Option<&str>,
    ) -> Self {
        self.apply(|editor| {
            let old_name = OptionallyQualifiedName::unquoted(old_table_name, qualifier)?;
            let new_name = UnqualifiedName::unquoted(new_table_name)?;
            editor.rename_table_inner(&old_name, &new_name)
        })
    }

    #[must_use]
    pub fn drop_table(self, table_name: &OptionallyQualifiedName) -> Self {
        self.apply(|editor| editor.drop_table_inner(table_name))
    }

    fn drop_table_inner(&mut self, table_name: &OptionallyQualifiedName) -> Result<(), Exception> {
        let key = self.get_key(table_name)?;

        if self.tables.remove(&key).is_none() {
            return Err(exception::schema_table_does_not_exist(table_name));
        }

        Ok(())
    }

    #[must_use]
    pub fn drop_table_by_unquoted_name(self, table_name: &str, qualifier: Option<&str>) -> Self {
        self.apply(|editor| {
            let name = OptionallyQualifiedName::unquoted(table_name, qualifier)?;
            editor.drop_table_inner(&name)
        })
    }

    #[must_use]
    pub fn set_sequences(self, sequences: Vec<Sequence>) -> Self {
        self.apply(|editor| {
            editor.sequences.clear();
            for sequence in sequences {
                editor.add_sequence_inner(sequence)?;
            }
            Ok(())
        })
    }

    #[must_use]
    pub fn add_sequence(self, sequence: Sequence) -> Self {
        self.apply(|editor| editor.add_sequence_inner(sequence))
    }

    fn add_sequence_inner(&mut self, sequence: Sequence) -> Result<(), Exception> {
        let name = sequence.get_object_name()?;
        let key = self.get_key(&name)?;

        if self.sequences.contains_key(&key) {
            return Err(exception::schema_sequence_already_exists(&name));
        }

        self.sequences.insert(key, sequence);

        Ok(())
    }

    #[must_use]
    pub fn drop_sequence(self, sequence_name: &OptionallyQualifiedName) -> Self {
        self.apply(|editor| editor.drop_sequence_inner(sequence_name))
    }

    fn drop_sequence_inner(
        &mut self,
        sequence_name: &OptionallyQualifiedName,
    ) -> Result<(), Exception> {
        let key = self.get_key(sequence_name)?;

        if self.sequences.remove(&key).is_none() {
            return Err(exception::schema_sequence_does_not_exist(sequence_name));
        }

        Ok(())
    }

    #[must_use]
    pub fn drop_sequence_by_unquoted_name(
        self,
        sequence_name: &str,
        qualifier: Option<&str>,
    ) -> Self {
        self.apply(|editor| {
            let name = OptionallyQualifiedName::unquoted(sequence_name, qualifier)?;
            editor.drop_sequence_inner(&name)
        })
    }

    pub fn create(self) -> Result<Schema, Exception> {
        if let Some(e) = self.error {
            return Err(e);
        }

        self.ensure_uniform_qualification()?;

        let mut schema_config = SchemaConfig::new();
        if let Some(name) = &self.default_namespace_name {
            schema_config.set_name(Some(name));
        }

        Schema::new(
            self.tables.values().cloned().collect(),
            self.sequences.values().cloned().collect(),
            Some(schema_config),
            &[],
        )
    }

    fn get_key(&self, name: &OptionallyQualifiedName) -> Result<String, Exception> {
        Ok(build_key(&self.resolve_name(name)?))
    }

    /// `resolveName($name)`: the name qualified with the default namespace
    /// if one is configured and the name is unqualified; otherwise the
    /// name unchanged.
    fn resolve_name(
        &self,
        name: &OptionallyQualifiedName,
    ) -> Result<OptionallyQualifiedName, Exception> {
        if name.get_qualifier().is_some() {
            return Ok(name.clone());
        }

        let Some(default_namespace_name) = &self.default_namespace_name else {
            return Ok(name.clone());
        };

        Ok(OptionallyQualifiedName::new(
            name.get_unqualified_name().clone(),
            Some(Identifier::quoted(default_namespace_name)?),
        ))
    }

    /// Fails if the editor's contents mix qualified and unqualified names
    /// after resolution against the default namespace. Tables and
    /// sequences are checked together.
    fn ensure_uniform_qualification(&self) -> Result<(), Exception> {
        let mut qualified = false;
        let mut unqualified = false;

        for table in self.tables.values() {
            self.check_qualification(&table.get_object_name()?, &mut qualified, &mut unqualified)?;
        }

        for sequence in self.sequences.values() {
            self.check_qualification(
                &sequence.get_object_name()?,
                &mut qualified,
                &mut unqualified,
            )?;
        }

        Ok(())
    }

    fn check_qualification(
        &self,
        name: &OptionallyQualifiedName,
        qualified: &mut bool,
        unqualified: &mut bool,
    ) -> Result<(), Exception> {
        if self.resolve_name(name)?.get_qualifier().is_some() {
            if *unqualified {
                return Err(exception::improperly_qualified_from_qualified_name(name));
            }

            *qualified = true;
        } else {
            if *qualified {
                return Err(exception::improperly_qualified_from_unqualified_name(name));
            }

            *unqualified = true;
        }

        Ok(())
    }
}

/// `qualifiersEqual($a, $b)`: `strcasecmp()` of the values.
fn qualifiers_equal(a: Option<&Identifier>, b: Option<&Identifier>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.get_value().eq_ignore_ascii_case(b.get_value()),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// `buildKey($resolved)`.
fn build_key(resolved: &OptionallyQualifiedName) -> String {
    let unqualified_name = resolved.get_unqualified_name().get_value();

    match resolved.get_qualifier() {
        None => unqualified_name.to_ascii_lowercase(),
        Some(qualifier) => format!(
            "{}\0{}",
            qualifier.get_value().to_ascii_lowercase(),
            unqualified_name.to_ascii_lowercase()
        ),
    }
}
