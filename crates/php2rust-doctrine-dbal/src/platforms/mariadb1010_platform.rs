//! Port of Doctrine\DBAL\Platforms\MariaDB1010Platform.php:
//! a `MariaDB1060Platform`.
//!
//! `fetchTableOptionsByTable()` (`FULL_COLLATION_NAME`) is a
//! `MySQLSchemaManager` helper and is not ported (the schema manager is
//! not).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MariaDB1010Platform.php

use crate::exception::Exception;
use crate::schema::Index;

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::AbstractPlatform;
use super::keywords::{KeywordList, MariaDBKeywords};
use super::mariadb_platform::impl_mariadb_platform;
use super::mysql_platform;
use super::platform_state::PlatformState;

/// Provides the behavior, features and SQL dialect of the `MariaDB` 10.10
/// database platform.
#[derive(Debug, Default)]
pub struct MariaDB1010Platform {
    state: PlatformState,
}

impl MariaDB1010Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MariaDB1010Platform";

    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPlatform for MariaDB1010Platform {
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

    // createSelectSQLBuilder(): AbstractPlatform's (SKIP LOCKED).
}
