//! Port of Doctrine\DBAL\Platforms\MySQLPlatform.php: the Oracle `MySQL`
//! platform of the oldest supported version.
//!
//! The deprecation `createReservedKeywordsList()` raises when called from
//! outside the package is not raised (the package calls it itself).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQLPlatform.php

use php_value::{PhpArray, Value};

use crate::exception::Exception;
use crate::schema::Index;
use crate::sql::builder::{DefaultSelectSQLBuilder, SelectSQLBuilder, WithSQLBuilder};

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::{AbstractPlatform, parent};
use super::exception::NotSupported;
use super::keywords::{KeywordList, MySQLKeywords};
use super::platform_state::PlatformState;

/// Provides the behavior, features and SQL dialect of the Oracle `MySQL`
/// database platform of the oldest supported version.
#[derive(Debug, Default)]
pub struct MySQLPlatform {
    state: PlatformState,
}

impl MySQLPlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MySQLPlatform";

    pub fn new() -> Self {
        Self::default()
    }
}

/// `getDefaultValueDeclarationSQL()`: Oracle `MySQL` does not support
/// default values on TEXT/BLOB columns until 8.0.13.
pub(crate) fn get_default_value_declaration_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    column: &PhpArray,
) -> Result<String, Exception> {
    let type_ = this.get_column_type_or_null(column)?;
    if let Some(type_) = type_
        && matches!(
            type_.class(),
            "Doctrine\\DBAL\\Types\\TextType" | "Doctrine\\DBAL\\Types\\BlobType"
        )
    {
        let mut column = column.clone();
        column.insert("default", Value::Null);
        return parent::get_default_value_declaration_sql(this, &column);
    }
    parent::get_default_value_declaration_sql(this, column)
}

/// `getRenameIndexSQL()`.
pub(crate) fn get_rename_index_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    old_index_name: &str,
    index: &Index,
    table_name: &str,
) -> Vec<String> {
    vec![format!(
        "ALTER TABLE {table_name} RENAME INDEX {old_index_name} TO {}",
        index.get_quoted_name(this)
    )]
}

impl AbstractPlatform for MySQLPlatform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    impl_abstract_mysql_platform!();

    fn get_default_value_declaration_sql(&self, column: &PhpArray) -> Result<String, Exception> {
        get_default_value_declaration_sql(self, column)
    }

    fn create_with_sql_builder(&self) -> Result<WithSQLBuilder, Exception> {
        Err(
            NotSupported::new("Doctrine\\DBAL\\Platforms\\MySQLPlatform::createWithSQLBuilder")
                .into(),
        )
    }

    fn get_rename_index_sql(
        &self,
        old_index_name: &str,
        index: &Index,
        table_name: &str,
    ) -> Result<Vec<String>, Exception> {
        Ok(get_rename_index_sql(
            self,
            old_index_name,
            index,
            table_name,
        ))
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        MySQLKeywords::new()
    }

    /// `AbstractMySQLPlatform::createSelectSQLBuilder()`: no SKIP LOCKED.
    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(self, Some("FOR UPDATE"), None))
    }
}
