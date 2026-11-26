//! Port of Doctrine\DBAL\Platforms\SQLite\Comparator.php: compares
//! schemas in the context of `SQLite` platform.
//!
//! PHP's subclass is a newtype over `schema::Comparator` (its methods by
//! `Deref`) whose `compareTables()` normalizes both tables first.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/SQLite/Comparator.php

use std::ops::Deref;

use php_value::{Key, Value};

use crate::exception::Exception;
use crate::platforms::SQLitePlatform;
use crate::schema::{Comparator as BaseComparator, ComparatorConfig, Table};

/// Compares schemas in the context of `SQLite` platform.
///
/// BINARY is the default column collation and should be ignored if
/// specified explicitly.
#[derive(Debug)]
pub struct Comparator<'a>(BaseComparator<'a>);

impl<'a> Comparator<'a> {
    /// `@internal` The comparator can be only instantiated by a schema
    /// manager.
    pub fn new(platform: &'a SQLitePlatform, config: ComparatorConfig) -> Self {
        Self(BaseComparator::with_normalizer(
            platform,
            config,
            Box::new(normalize_columns),
        ))
    }
}

impl<'a> Deref for Comparator<'a> {
    type Target = BaseComparator<'a>;

    fn deref(&self) -> &BaseComparator<'a> {
        &self.0
    }
}

/// `normalizeColumns($table)`.
fn normalize_columns(table: &Table) -> Result<Table, Exception> {
    let mut table = table.clone();

    for column in table.columns_mut() {
        let mut options = column.get_platform_options().clone();

        let is_binary = match options.get(&Key::from("collation")) {
            None | Some(Value::Null) => false,
            Some(Value::String(collation)) => collation.as_bytes().eq_ignore_ascii_case(b"binary"),
            Some(other) => {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    format!(
                        "strcasecmp(): Argument #1 ($string1) must be of type string, {} given",
                        other.value_name()
                    ),
                ));
            }
        };
        if !is_binary {
            continue;
        }

        options.remove(&Key::from("collation"));
        column.set_platform_options_inner(options);
    }

    Ok(table)
}
