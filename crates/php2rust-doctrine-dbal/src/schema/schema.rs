//! Port of Doctrine\DBAL\Schema\Schema.php: the object representation of
//! a database schema.
//!
//! PHP hands out the schema's tables and sequences by handle; here the
//! getters borrow them, and `create_table()` / `create_sequence()` return
//! the stored object mutably. `__clone()`'s deep clone is `Clone`. The
//! constructor takes the tables and sequences by value (in PHP it calls
//! `setSchemaConfig()` on the caller's handles).
//!
//! `Schema` overrides `AbstractAsset::setName()` with a method that does
//! nothing, so its asset state is kept as an optionally named object
//! (whose `setName()` never fails either) with the unqualified name
//! parser.
//!
//! `toSql()` and `toDropSql()` are not ported: they need
//! `CreateSchemaObjectsSQLBuilder` and `DropSchemaObjectsSQLBuilder`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Schema.php

use std::ops::Deref;

use php_value::{Key, Value};

use crate::deprecations;
use crate::exception::Exception;

use super::abstract_asset::{
    AbstractAsset, AssetKind, NameParser, is_identifier_quoted, trim_quotes,
};
use super::array_map::ArrayMap;
use super::exception;
use super::identifier::Identifier;
use super::schema_config::SchemaConfig;
use super::schema_editor::SchemaEditor;
use super::sequence::Sequence;
use super::table::{Table, TableParts};

const LINK_EDIT: &str = "https://github.com/doctrine/dbal/pull/7373";
const LINK_NAMESPACES: &str = "https://github.com/doctrine/dbal/pull/7186";

/// Object representation of a database schema.
#[derive(Debug, Clone)]
pub struct Schema {
    asset: AbstractAsset,
    /// The namespaces in this schema.
    namespaces: ArrayMap<String>,
    tables: ArrayMap<Table>,
    sequences: ArrayMap<Sequence>,
    config: SchemaConfig,
    /// Indicates whether the schema uses unqualified names for its
    /// objects. Once this flag is set to true, it won't be unset even
    /// after the objects with unqualified names have been dropped from
    /// the schema.
    uses_unqualified_names: bool,
}

impl Schema {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\Schema";

    /// `new Schema($tables, $sequences, $schemaConfig, $namespaces)`
    /// (`@internal` since doctrine/dbal 4.5: use `Schema::editor()`).
    pub fn new(
        tables: Vec<Table>,
        sequences: Vec<Sequence>,
        schema_config: Option<SchemaConfig>,
        namespaces: &[&str],
    ) -> Result<Self, Exception> {
        if !namespaces.is_empty() {
            deprecations::trigger_if_called_from_outside(LINK_NAMESPACES, || {
                "Passing the $namespaces argument to the Schema constructor is deprecated."
                    .to_owned()
            });
        }

        let schema_config = schema_config.unwrap_or_default();

        let name = schema_config.get_name().unwrap_or("").to_owned();

        let mut schema = Self {
            asset: AbstractAsset::new(
                Self::CLASS,
                AssetKind::OptionallyNamed,
                NameParser::Unqualified,
                Some(&name),
            ),
            namespaces: ArrayMap::new(),
            tables: ArrayMap::new(),
            sequences: ArrayMap::new(),
            config: schema_config,
            uses_unqualified_names: false,
        };

        for namespace in namespaces {
            schema.create_namespace_inner(namespace)?;
        }

        for mut table in tables {
            table.set_schema_config_inner(schema.config.clone());
            schema.add_table_inner(table)?;
        }

        for sequence in sequences {
            schema.add_sequence_inner(sequence)?;
        }

        Ok(schema)
    }

    /// `getName()` (deprecated).
    pub fn get_name(&self) -> String {
        deprecations::trigger_if_called_from_outside(
            "https://github.com/doctrine/dbal/pull/6734",
            || {
                "Using Schema as AbstractAsset, including Doctrine\\DBAL\\Schema\\Schema::getName, is deprecated.".to_owned()
            },
        );
        self.asset.get_name()
    }

    /// `_addTable($table)`; returns the key the table is stored under.
    fn add_table_inner(&mut self, table: Table) -> Result<String, Exception> {
        let resolved_name = self.resolve_name(&table);

        let key = self.get_key_from_resolved_name(&resolved_name);

        if self.tables.contains_key(&key) {
            return Err(exception::table_already_exists(&resolved_name.get_name()));
        }

        match resolved_name.get_namespace_name() {
            Some(namespace_name) => {
                if !table.is_in_default_namespace(&self.asset.get_name())
                    && !self.has_namespace(namespace_name)
                {
                    self.create_namespace_inner(namespace_name)?;
                }
            }
            None => self.uses_unqualified_names = true,
        }

        self.tables.insert(key.clone(), table);

        Ok(key)
    }

    /// `_addSequence($sequence)`; returns the key the sequence is stored
    /// under.
    fn add_sequence_inner(&mut self, sequence: Sequence) -> Result<String, Exception> {
        let resolved_name = self.resolve_name(&sequence);

        let key = self.get_key_from_resolved_name(&resolved_name);

        if self.sequences.contains_key(&key) {
            return Err(exception::sequence_already_exists(
                &resolved_name.get_name(),
            ));
        }

        match resolved_name.get_namespace_name() {
            Some(namespace_name) => {
                if !sequence.is_in_default_namespace(&self.asset.get_name())
                    && !self.has_namespace(namespace_name)
                {
                    self.create_namespace_inner(namespace_name)?;
                }
            }
            None => self.uses_unqualified_names = true,
        }

        self.sequences.insert(key.clone(), sequence);

        Ok(key)
    }

    /// Returns the namespaces of this schema.
    pub fn get_namespaces(&self) -> Vec<String> {
        self.namespaces.values().cloned().collect()
    }

    /// Gets all tables of this schema.
    pub fn get_tables(&self) -> Vec<&Table> {
        self.tables.values().collect()
    }

    /// `getTable($name)`.
    pub fn get_table(&self, name: &str) -> Result<&Table, Exception> {
        let key = self.get_key_from_name(name);
        self.tables
            .get(&key)
            .ok_or_else(|| exception::table_does_not_exist(name))
    }

    /// `getTable($name)`, for changing the table's handle.
    pub fn get_table_mut(&mut self, name: &str) -> Result<&mut Table, Exception> {
        let key = self.get_key_from_name(name);
        self.tables
            .get_mut(&key)
            .ok_or_else(|| exception::table_does_not_exist(name))
    }

    /// `getKeyFromResolvedName($asset)`: the lower-cased full object name
    /// under which the object is kept.
    fn get_key_from_resolved_name(&self, asset: &AbstractAsset) -> String {
        let mut key = asset.get_name();

        if asset.get_namespace_name().is_some() {
            if self.uses_unqualified_names {
                deprecations::trigger(
                    "https://github.com/doctrine/dbal/pull/6677#user-content-qualified-names",
                    || {
                        "Using qualified names to create or reference objects in a schema that uses unqualified names is deprecated.".to_owned()
                    },
                );
            }

            key = format!("{}.{key}", self.asset.get_name());
        } else if !self.namespaces.is_empty() {
            deprecations::trigger(
                "https://github.com/doctrine/dbal/pull/6677#user-content-unqualified-names",
                || {
                    "Using unqualified names to create or reference objects in a schema that uses qualified names and lacks a default namespace configuration is deprecated.".to_owned()
                },
            );
        }

        key.to_ascii_lowercase()
    }

    /// `getKeyFromName($name)`.
    fn get_key_from_name(&self, name: &str) -> String {
        let identifier = Identifier::new(name, false);
        self.get_key_from_resolved_name(&self.resolve_name(&identifier))
    }

    /// `resolveName($asset)`: the name qualified with the schema name when
    /// it is unqualified and the schema has one.
    fn resolve_name(&self, asset: &AbstractAsset) -> AbstractAsset {
        if asset.get_namespace_name().is_none() {
            let default_namespace_name = self.asset.get_name();

            if !default_namespace_name.is_empty() {
                let identifier = Identifier::new(
                    &format!("{default_namespace_name}.{}", asset.get_name()),
                    false,
                );
                return (*identifier).clone();
            }
        }

        asset.clone()
    }

    /// `getUnquotedAssetName($assetName)`.
    fn get_unquoted_asset_name(asset_name: &str) -> String {
        if is_identifier_quoted(asset_name) {
            return trim_quotes(asset_name);
        }
        asset_name.to_owned()
    }

    /// Does this schema have a namespace with the given name?
    pub fn has_namespace(&self, name: &str) -> bool {
        let name = Self::get_unquoted_asset_name(name).to_ascii_lowercase();
        self.namespaces.contains_key(&name)
    }

    /// Does this schema have a table with the given name?
    pub fn has_table(&self, name: &str) -> bool {
        let key = self.get_key_from_name(name);
        self.tables.contains_key(&key)
    }

    pub fn has_sequence(&self, name: &str) -> bool {
        let key = self.get_key_from_name(name);
        self.sequences.contains_key(&key)
    }

    /// `getSequence($name)`.
    pub fn get_sequence(&self, name: &str) -> Result<&Sequence, Exception> {
        let key = self.get_key_from_name(name);
        self.sequences
            .get(&key)
            .ok_or_else(|| exception::sequence_does_not_exist(name))
    }

    pub fn get_sequences(&self) -> Vec<&Sequence> {
        self.sequences.values().collect()
    }

    /// `createNamespace($name)` (deprecated: the schema derives its
    /// namespaces from the names of its tables and sequences).
    pub fn create_namespace(&mut self, name: &str) -> Result<&mut Self, Exception> {
        deprecations::trigger_if_called_from_outside(LINK_NAMESPACES, || {
            "Doctrine\\DBAL\\Schema\\Schema::createNamespace is deprecated. The schema automatically derives namespaces from the names of its tables and sequences. Creating empty namespaces is deprecated.".to_owned()
        });
        self.create_namespace_inner(name)?;
        Ok(self)
    }

    fn create_namespace_inner(&mut self, name: &str) -> Result<(), Exception> {
        let unquoted_name = Self::get_unquoted_asset_name(name).to_ascii_lowercase();

        if self.namespaces.contains_key(&unquoted_name) {
            return Err(exception::namespace_already_exists(&unquoted_name));
        }

        self.namespaces.insert(unquoted_name, name.to_owned());

        Ok(())
    }

    /// `createTable($name)` (deprecated: use `edit()` and
    /// `SchemaEditor::addTable()`).
    pub fn create_table(&mut self, name: &str) -> Result<&mut Table, Exception> {
        deprecations::trigger(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Schema::createTable is deprecated. Use Schema::edit() and SchemaEditor::addTable() instead.".to_owned()
        });

        let table = Table::with_parts(
            name,
            TableParts {
                configuration: Some(self.config.to_table_configuration()),
                ..TableParts::default()
            },
        )?;
        let key = self.add_table_inner(table)?;

        let options: Vec<(Key, Value)> = self
            .config
            .get_default_table_options()
            .iter()
            .map(|(option, value)| (option.clone(), value.clone()))
            .collect();

        let table = self
            .tables
            .get_mut(&key)
            .ok_or_else(|| exception::table_does_not_exist(name))?;

        for (option, value) in options {
            // `addOption(string $name, ...)` under strict_types.
            if let Key::Int(_) = option {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    "Doctrine\\DBAL\\Schema\\Table::addOption(): Argument #1 ($name) must be of type string, int given".to_owned(),
                ));
            }
            table.add_option_key(option, value);
        }

        Ok(table)
    }

    /// `renameTable($oldName, $newName)` (deprecated: use `edit()` and
    /// `SchemaEditor::renameTable()`).
    pub fn rename_table(&mut self, old_name: &str, new_name: &str) -> Result<&mut Self, Exception> {
        deprecations::trigger(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Schema::renameTable is deprecated. Use Schema::edit() and SchemaEditor::renameTable() instead.".to_owned()
        });

        let mut table = self.get_table(old_name)?.clone();

        let identifier = Identifier::new(new_name, false);

        let asset = table.asset_mut();
        asset.name.clone_from(&identifier.name);
        asset.namespace.clone_from(&identifier.namespace);
        asset.quoted = identifier.quoted;

        self.drop_table_inner(old_name)?;
        self.add_table_inner(table)?;

        Ok(self)
    }

    /// `dropTable($name)` (deprecated: use `edit()` and
    /// `SchemaEditor::dropTable()`).
    pub fn drop_table(&mut self, name: &str) -> Result<&mut Self, Exception> {
        deprecations::trigger_if_called_from_outside(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Schema::dropTable is deprecated. Use Schema::edit() and SchemaEditor::dropTable() instead.".to_owned()
        });
        self.drop_table_inner(name)?;
        Ok(self)
    }

    fn drop_table_inner(&mut self, name: &str) -> Result<(), Exception> {
        let key = self.get_key_from_name(name);
        if self.tables.remove(&key).is_none() {
            return Err(exception::table_does_not_exist(name));
        }
        Ok(())
    }

    /// `createSequence($name, $allocationSize, $initialValue)`
    /// (deprecated: use `edit()` and `SchemaEditor::addSequence()`).
    pub fn create_sequence(
        &mut self,
        name: &str,
        allocation_size: i64,
        initial_value: i64,
    ) -> Result<&mut Sequence, Exception> {
        deprecations::trigger(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Schema::createSequence is deprecated. Use Schema::edit() and SchemaEditor::addSequence() instead.".to_owned()
        });

        let sequence = Sequence::with_options(name, allocation_size, initial_value, None);
        let key = self.add_sequence_inner(sequence)?;

        self.sequences
            .get_mut(&key)
            .ok_or_else(|| exception::sequence_does_not_exist(name))
    }

    /// `dropSequence($name)` (deprecated: use `edit()` and
    /// `SchemaEditor::dropSequence()`).
    pub fn drop_sequence(&mut self, name: &str) -> &mut Self {
        deprecations::trigger(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Schema::dropSequence is deprecated. Use Schema::edit() and SchemaEditor::dropSequence() instead.".to_owned()
        });

        let key = self.get_key_from_name(name);
        self.sequences.remove(&key);

        self
    }

    /// `Schema::editor()`.
    pub fn editor() -> SchemaEditor {
        SchemaEditor::new()
    }

    /// `edit()`: an editor seeded with this schema's tables, sequences and
    /// configuration.
    pub fn edit(&self) -> SchemaEditor {
        Self::editor()
            .set_default_namespace(self.config.get_name())
            .set_tables(self.tables.values().cloned().collect())
            .set_sequences(self.sequences.values().cloned().collect())
    }
}

impl Deref for Schema {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}
