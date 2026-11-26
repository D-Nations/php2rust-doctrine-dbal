//! Port of Doctrine\DBAL\Schema\Sequence.php: a sequence structure.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Sequence.php

use std::ops::Deref;

use crate::deprecations;
use crate::exception::Exception;

use super::abstract_asset::{AbstractAsset, AssetKind, NameParser};
use super::exception::invalid_state;
use super::name::{Name, OptionallyQualifiedName};
use super::sequence_editor::SequenceEditor;
use super::table::Table;

const LINK_EDIT: &str = "https://github.com/doctrine/dbal/pull/7115";
const LINK_CACHE: &str = "https://github.com/doctrine/dbal/pull/7108";

/// `Doctrine\DBAL\Schema\Sequence`.
#[derive(Debug, Clone, PartialEq)]
pub struct Sequence {
    asset: AbstractAsset,
    allocation_size: i64,
    initial_value: i64,
    cache: Option<i64>,
}

impl Sequence {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\Sequence";

    /// `new Sequence($name)` with the default arguments.
    pub fn new(name: &str) -> Self {
        Self::with_options(name, 1, 1, None)
    }

    /// `new Sequence($name, $allocationSize, $initialValue, $cache)`.
    pub fn with_options(
        name: &str,
        allocation_size: i64,
        initial_value: i64,
        cache: Option<i64>,
    ) -> Self {
        let asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::Named,
            NameParser::OptionallyQualified,
            Some(name),
        );

        // `$cache < 0`: null is not less than 0.
        if cache.is_some_and(|cache| cache < 0) {
            deprecations::trigger_if_called_from_outside(LINK_CACHE, || {
                "Passing a negative value as sequence cache size is deprecated.".to_owned()
            });
        }

        Self {
            asset,
            allocation_size,
            initial_value,
            cache,
        }
    }

    /// `getObjectName()`.
    pub fn get_object_name(&self) -> Result<OptionallyQualifiedName, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::OptionallyQualified(name)) => Ok(name.clone()),
            _ => Err(invalid_state::object_name_not_initialized()),
        }
    }

    pub fn get_allocation_size(&self) -> i64 {
        self.allocation_size
    }

    pub fn get_initial_value(&self) -> i64 {
        self.initial_value
    }

    /// `getCache()` (deprecated: use [`Self::get_cache_size`]).
    pub fn get_cache(&self) -> Option<i64> {
        deprecations::trigger_if_called_from_outside(LINK_CACHE, || {
            "Doctrine\\DBAL\\Schema\\Sequence::getCache is deprecated, use `getCacheSize()` instead."
                .to_owned()
        });
        self.cache
    }

    /// `getCacheSize()`.
    pub fn get_cache_size(&self) -> Option<i64> {
        self.cache
    }

    /// `setAllocationSize($allocationSize)` (deprecated).
    pub fn set_allocation_size(&mut self, allocation_size: i64) -> &mut Self {
        deprecations::trigger_if_called_from_outside(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Sequence::setAllocationSize is deprecated. Use Sequence::edit() and SequenceEditor::setAllocationSize() instead.".to_owned()
        });
        self.allocation_size = allocation_size;
        self
    }

    /// `setInitialValue($initialValue)` (deprecated).
    pub fn set_initial_value(&mut self, initial_value: i64) -> &mut Self {
        deprecations::trigger_if_called_from_outside(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Sequence::setInitialValue is deprecated. Use Sequence::edit() and SequenceEditor::setInitialValue() instead.".to_owned()
        });
        self.initial_value = initial_value;
        self
    }

    /// `setCache($cache)` (deprecated).
    pub fn set_cache(&mut self, cache: i64) -> &mut Self {
        deprecations::trigger(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Sequence::setCache is deprecated. Use Sequence::edit() and SequenceEditor::setCacheSize() instead.".to_owned()
        });
        self.cache = Some(cache);
        self
    }

    /// `isAutoIncrementsFor($table)` (deprecated): checks if this
    /// sequence is an autoincrement sequence for a given table.
    pub fn is_auto_increments_for(&self, table: &Table) -> Result<bool, Exception> {
        deprecations::trigger_if_called_from_outside(
            "https://github.com/doctrine/dbal/pull/6654",
            || {
                "Doctrine\\DBAL\\Schema\\Sequence::isAutoIncrementsFor is deprecated and will be removed in 5.0.".to_owned()
            },
        );
        self.is_auto_increments_for_inner(table)
    }

    pub(crate) fn is_auto_increments_for_inner(&self, table: &Table) -> Result<bool, Exception> {
        let Some(primary_key) = table.get_primary_key() else {
            return Ok(false);
        };

        let pk_columns = primary_key.get_columns();

        let [pk_column] = pk_columns.as_slice() else {
            return Ok(false);
        };

        let column = table.get_column(pk_column)?;

        if !column.get_autoincrement() {
            return Ok(false);
        }

        let namespace = table.get_namespace_name();
        let sequence_name = self.get_shortest_name(namespace);
        let table_name = table.get_shortest_name(namespace);
        let table_sequence_name =
            format!("{table_name}_{}_seq", column.get_shortest_name(namespace));

        Ok(table_sequence_name == sequence_name)
    }

    /// `Sequence::editor()`.
    pub fn editor() -> SequenceEditor {
        SequenceEditor::new()
    }

    /// `edit()`: an editor initialized with the sequence's properties.
    pub fn edit(&self) -> Result<SequenceEditor, Exception> {
        Ok(Self::editor()
            .set_name(self.get_object_name()?)
            .set_allocation_size(self.allocation_size)
            .set_initial_value(self.initial_value)
            .set_cache_size(self.cache))
    }
}

impl Deref for Sequence {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}
