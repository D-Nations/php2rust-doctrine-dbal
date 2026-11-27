//! Port of Doctrine\DBAL\Platforms\MariaDB1052Platform.php (deprecated upstream):
//! a `MariaDBPlatform` that renames indexes with
//! `RENAME INDEX` and leaves the foreign keys of renamed indexes alone.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MariaDB1052Platform.php

use crate::exception::Exception;
use crate::schema::Index;

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::AbstractPlatform;
use super::keywords::{KeywordList, MariaDBKeywords};
use super::mariadb_platform::impl_mariadb_platform;
use super::mysql_platform;
use super::platform_state::PlatformState;
use crate::sql::builder::{DefaultSelectSQLBuilder, SelectSQLBuilder};

/// Provides the behavior, features and SQL dialect of the `MariaDB` 10.5
/// database platform.
#[derive(Debug, Default)]
pub struct MariaDB1052Platform {
    state: PlatformState,
}

impl MariaDB1052Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MariaDB1052Platform";

    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPlatform for MariaDB1052Platform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    impl_abstract_mysql_platform!();

    impl_mariadb_platform!();

    // getPreAlterTableRenameIndexForeignKeySQL() and
    // getPostAlterTableIndexForeignKeySQL(): AbstractMySQLPlatform's.

    /// `MariaDB1052Platform::getRenameIndexSQL()`.
    fn get_rename_index_sql(
        &self,
        old_index_name: &str,
        index: &Index,
        table_name: &str,
    ) -> Result<Vec<String>, Exception> {
        Ok(mysql_platform::get_rename_index_sql(
            self,
            old_index_name,
            index,
            table_name,
        ))
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        MariaDBKeywords::new()
    }

    /// `AbstractMySQLPlatform::createSelectSQLBuilder()`: no SKIP LOCKED.
    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(self, Some("FOR UPDATE"), None))
    }
}
