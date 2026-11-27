//! Port of Doctrine\DBAL\Platforms\SQLServer\Comparator.php: compares
//! schemas in the context of SQL Server platform.
//!
//! PHP's subclass is a newtype over `schema::Comparator` (its methods by
//! `Deref`) whose `compareTables()` normalizes both tables first.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/SQLServer/Comparator.php

use std::ops::Deref;

use php_value::{Key, Value};

use crate::platforms::SQLServerPlatform;
use crate::schema::{Comparator as BaseComparator, ComparatorConfig, Table};

/// Compares schemas in the context of SQL Server platform: a column
/// collation equal to the database's is the same as none.
#[derive(Debug)]
pub struct Comparator<'a>(BaseComparator<'a>);

impl<'a> Comparator<'a> {
    /// `@internal` The comparator can be only instantiated by a schema
    /// manager.
    pub fn new(
        platform: &'a SQLServerPlatform,
        database_collation: &str,
        config: ComparatorConfig,
    ) -> Self {
        let database_collation = database_collation.to_owned();
        Self(BaseComparator::with_normalizer(
            platform,
            config,
            Box::new(move |table| Ok(normalize_columns(table, &database_collation))),
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
fn normalize_columns(table: &Table, database_collation: &str) -> Table {
    let mut table = table.clone();

    for column in table.columns_mut() {
        let mut options = column.get_platform_options().clone();

        let matches = matches!(
            options.get(&Key::from("collation")),
            Some(Value::String(collation)) if collation.as_bytes() == database_collation.as_bytes()
        );
        if !matches {
            continue;
        }

        options.remove(&Key::from("collation"));
        column.set_platform_options_inner(options);
    }

    table
}
