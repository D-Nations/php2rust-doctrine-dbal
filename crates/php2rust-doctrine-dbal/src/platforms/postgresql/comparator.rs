//! Port of Doctrine\DBAL\Platforms\PostgreSQL\Comparator.php: compares
//! schemas in the context of `PostgreSQL` platform.
//!
//! `PostgreSQL` treats "default" collation as implicit, so an explicit
//! "default" value should be ignored when comparing schemas. PHP's
//! subclass is a newtype over `schema::Comparator` (its methods by
//! `Deref`) whose `compareTables()` normalizes both tables first.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/PostgreSQL/Comparator.php

use std::ops::Deref;

use php_value::{Key, Value};

use crate::platforms::PostgreSQLPlatform;
use crate::schema::{Comparator as BaseComparator, ComparatorConfig, Table};

/// Compares schemas in the context of `PostgreSQL` platform: an explicit
/// "default" column collation is the same as none.
#[derive(Debug)]
pub struct Comparator<'a>(BaseComparator<'a>);

impl<'a> Comparator<'a> {
    /// `new Comparator($platform, $config)`.
    pub fn new(platform: &'a PostgreSQLPlatform, config: ComparatorConfig) -> Self {
        Self(BaseComparator::with_normalizer(
            platform,
            config,
            Box::new(|table| Ok(normalize_columns(table))),
        ))
    }
}

impl<'a> Deref for Comparator<'a> {
    type Target = BaseComparator<'a>;

    fn deref(&self) -> &BaseComparator<'a> {
        &self.0
    }
}

/// `normalizeColumns($table)` (private).
fn normalize_columns(table: &Table) -> Table {
    let mut table = table.clone();

    for column in table.columns_mut() {
        let mut options = column.get_platform_options().clone();

        let is_default = matches!(
            options.get(&Key::from("collation")),
            Some(Value::String(collation)) if collation.as_bytes() == b"default"
        );
        if !is_default {
            continue;
        }

        options.remove(&Key::from("collation"));
        column.set_platform_options_inner(options);
    }

    table
}
