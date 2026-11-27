//! Port of Doctrine\DBAL\Platforms\MySQL84Platform.php (deprecated
//! upstream): the `MySQL` 8.4 platform, a `MySQL80Platform` with the 8.4
//! keywords.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL84Platform.php

use php_value::PhpArray;

use crate::exception::Exception;
use crate::schema::Index;

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::AbstractPlatform;
use super::keywords::{KeywordList, MySQL84Keywords};
use super::mysql_platform;
use super::platform_state::PlatformState;

/// Provides the behavior, features and SQL dialect of the `MySQL` 8.4
/// database platform.
#[derive(Debug, Default)]
pub struct MySQL84Platform {
    state: PlatformState,
}

impl MySQL84Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MySQL84Platform";

    pub fn new() -> Self {
        Self::default()
    }
}

impl AbstractPlatform for MySQL84Platform {
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
        MySQL84Keywords::new()
    }
}
