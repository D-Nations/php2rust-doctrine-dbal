//! Port of Doctrine\DBAL\Platforms\MariaDB120300Platform.php (deprecated upstream):
//! a `MariaDB110700Platform` with the 12.3 keywords.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MariaDB120300Platform.php

use crate::exception::Exception;
use crate::schema::Index;

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::AbstractPlatform;
use super::keywords::{KeywordList, MariaDB123Keywords};
use super::mariadb_platform::impl_mariadb_platform;
use super::mysql_platform;
use super::platform_state::PlatformState;

/// Provides the behavior, features and SQL dialect of the `MariaDB` 12.3
/// database platform.
#[derive(Debug, Default)]
pub struct MariaDB120300Platform {
    state: PlatformState,
}

impl MariaDB120300Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MariaDB120300Platform";

    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPlatform for MariaDB120300Platform {
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
        MariaDB123Keywords::new()
    }

    // createSelectSQLBuilder(): AbstractPlatform's (SKIP LOCKED).
}
