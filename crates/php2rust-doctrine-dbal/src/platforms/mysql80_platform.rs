//! Port of Doctrine\DBAL\Platforms\MySQL80Platform.php (deprecated
//! upstream): the `MySQL` 8.0 platform, a `MySQLPlatform` with the 8.0
//! keywords, SKIP LOCKED and WITH.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL80Platform.php

use php_value::PhpArray;

use crate::exception::Exception;
use crate::schema::Index;

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::AbstractPlatform;
use super::keywords::{KeywordList, MySQL80Keywords};
use super::mysql_platform;
use super::platform_state::PlatformState;

/// Provides the behavior, features and SQL dialect of the `MySQL` 8.0
/// database platform.
#[derive(Debug, Default)]
pub struct MySQL80Platform {
    state: PlatformState,
}

impl MySQL80Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MySQL80Platform";

    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPlatform for MySQL80Platform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    impl_abstract_mysql_platform!();

    /// `MySQLPlatform::getDefaultValueDeclarationSQL()`.
    fn get_default_value_declaration_sql(&self, column: &PhpArray) -> Result<String, Exception> {
        mysql_platform::get_default_value_declaration_sql(self, column)
    }

    /// `MySQLPlatform::getRenameIndexSQL()`.
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
        MySQL80Keywords::new()
    }

    // createSelectSQLBuilder() and createWithSQLBuilder(): AbstractPlatform's.
}
