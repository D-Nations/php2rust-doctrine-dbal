//! Port of Doctrine\DBAL\Platforms\MariaDB110700Platform.php (deprecated upstream):
//! a `MariaDB1010Platform` with the 11.7 keywords.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MariaDB110700Platform.php

use crate::exception::Exception;
use crate::schema::Index;

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::AbstractPlatform;
use super::keywords::{KeywordList, MariaDB117Keywords};
use super::mariadb_platform::impl_mariadb_platform;
use super::mysql_platform;
use super::platform_state::PlatformState;

/// Provides the behavior, features and SQL dialect of the `MariaDB` 11.7
/// database platform.
#[derive(Debug, Default)]
pub struct MariaDB110700Platform {
    state: PlatformState,
}

impl MariaDB110700Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MariaDB110700Platform";

    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPlatform for MariaDB110700Platform {
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
        MariaDB117Keywords::new()
    }

    // createSelectSQLBuilder(): AbstractPlatform's (SKIP LOCKED).
}
