//! Port of Doctrine\DBAL\Platforms\MariaDBPlatform.php: the `MariaDB`
//! platform of the oldest supported version.
//!
//! The free functions below are its methods, which the versioned
//! platforms (`MariaDB1052Platform` ...) inherit. `getColumnTypeSQLSnippet()`
//! is a `MySQLSchemaManager` helper and is not ported (the schema manager
//! is not). `in_array($foreignKey, $modifiedForeignKeys, true)` compares
//! foreign keys by value: PHP compares object identity. The deprecation
//! `createReservedKeywordsList()` raises when called from outside the
//! package is not raised (the package calls it itself).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MariaDBPlatform.php

use php_value::{Key, PhpArray};

use crate::exception::Exception;
use crate::schema::{ForeignKeyConstraint, TableDiff};

use super::abstract_mysql_platform::impl_abstract_mysql_platform;
use super::abstract_platform::{AbstractPlatform, parent};
use super::keywords::{KeywordList, MariaDBKeywords};
use super::platform_state::PlatformState;
use crate::sql::builder::{DefaultSelectSQLBuilder, SelectSQLBuilder};

/// Provides the behavior, features and SQL dialect of the `MariaDB`
/// database platform of the oldest supported version.
#[derive(Debug, Default)]
pub struct MariaDBPlatform {
    state: PlatformState,
}

impl MariaDBPlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\MariaDBPlatform";

    pub fn new() -> Self {
        Self::default()
    }
}

/// `getPreAlterTableRenameIndexForeignKeySQL()`: drops the remaining
/// foreign keys that use a renamed index.
pub(crate) fn get_pre_alter_table_rename_index_foreign_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
) -> Result<Vec<String>, Exception> {
    let mut sql = Vec::new();
    let table_name = diff.get_old_table().get_quoted_name(this);

    let modified_foreign_keys = diff.modified_foreign_keys();

    for foreign_key in get_remaining_foreign_key_constraints_requiring_renamed_indexes(diff) {
        if !modified_foreign_keys.contains(foreign_key) {
            sql.push(
                this.get_drop_foreign_key_sql(&foreign_key.get_quoted_name(this), &table_name)?,
            );
        }
    }

    Ok(sql)
}

/// `getPostAlterTableIndexForeignKeySQL()`.
pub(crate) fn get_post_alter_table_index_foreign_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
) -> Result<Vec<String>, Exception> {
    let mut sql = parent::get_post_alter_table_index_foreign_key_sql(this, diff)?;
    sql.extend(get_post_alter_table_rename_index_foreign_key_sql(
        this, diff,
    )?);
    Ok(sql)
}

/// `getPostAlterTableRenameIndexForeignKeySQL()` (private): recreates the
/// foreign keys the pre-alter statements dropped.
fn get_post_alter_table_rename_index_foreign_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
) -> Result<Vec<String>, Exception> {
    let mut sql = Vec::new();
    let table_name = diff.get_old_table().get_quoted_name(this);

    let modified_foreign_keys = diff.modified_foreign_keys();

    for foreign_key in get_remaining_foreign_key_constraints_requiring_renamed_indexes(diff) {
        if !modified_foreign_keys.contains(foreign_key) {
            sql.push(this.get_create_foreign_key_sql(foreign_key, &table_name)?);
        }
    }

    Ok(sql)
}

/// `getRemainingForeignKeyConstraintsRequiringRenamedIndexes()`
/// (private): the old table's foreign keys, less the dropped ones by key
/// (`array_diff_key()`), that intersect the columns of a renamed index.
fn get_remaining_foreign_key_constraints_requiring_renamed_indexes(
    diff: &TableDiff,
) -> Vec<&ForeignKeyConstraint> {
    let renamed_indexes = diff.renamed_indexes();

    if renamed_indexes.is_empty() {
        return Vec::new();
    }

    let mut foreign_keys = Vec::new();

    let dropped_keys: Vec<String> = diff
        .dropped_foreign_keys_by_key()
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let remaining_foreign_keys = diff
        .get_old_table()
        .get_foreign_keys()
        .iter()
        .filter(|(key, _)| !dropped_keys.iter().any(|dropped| dropped == key))
        .map(|(_, foreign_key)| foreign_key);

    for foreign_key in remaining_foreign_keys {
        for (_, index) in renamed_indexes {
            if foreign_key.intersects_index_columns_inner(index) {
                foreign_keys.push(foreign_key);
                break;
            }
        }
    }

    foreign_keys
}

/// `getColumnDeclarationSQL()`: no collation or character set for a JSON
/// column (`MariaDB` forces `utf8mb4_bin` on it). `instanceof JsonType`
/// is `JsonType` or `JsonbType` by class name.
pub(crate) fn get_column_declaration_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    name: &str,
    column: &PhpArray,
) -> Result<String, Exception> {
    // MariaDb forces column collation to utf8mb4_bin where the column was declared as JSON so ignore
    // collation and character set for json columns as attempting to set them can cause an error.
    if this
        .get_json_type_declaration_sql(&PhpArray::new())?
        .as_bytes()
        == b"JSON"
        && this.get_column_type_or_null(column)?.is_some_and(|type_| {
            matches!(
                type_.class(),
                "Doctrine\\DBAL\\Types\\JsonType" | "Doctrine\\DBAL\\Types\\JsonbType"
            )
        })
    {
        let mut column = column.clone();
        column.remove(&Key::from("collation"));
        column.remove(&Key::from("charset"));
        return parent::get_column_declaration_sql(this, name, &column);
    }

    parent::get_column_declaration_sql(this, name, column)
}

/// The methods every `MariaDB` platform has from `MariaDBPlatform`, for
/// the platform's `impl AbstractPlatform` (with
/// `impl_abstract_mysql_platform!()`).
macro_rules! impl_mariadb_platform {
    () => {
        fn get_column_declaration_sql(
            &self,
            name: &str,
            column: &php_value::PhpArray,
        ) -> Result<String, $crate::Exception> {
            $crate::platforms::mariadb_platform::get_column_declaration_sql(self, name, column)
        }
    };
}

pub(crate) use impl_mariadb_platform;

impl AbstractPlatform for MariaDBPlatform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    impl_abstract_mysql_platform!();

    impl_mariadb_platform!();

    fn get_pre_alter_table_rename_index_foreign_key_sql(
        &self,
        diff: &TableDiff,
    ) -> Result<Vec<String>, Exception> {
        get_pre_alter_table_rename_index_foreign_key_sql(self, diff)
    }

    fn get_post_alter_table_index_foreign_key_sql(
        &self,
        diff: &TableDiff,
    ) -> Result<Vec<String>, Exception> {
        get_post_alter_table_index_foreign_key_sql(self, diff)
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        MariaDBKeywords::new()
    }

    /// `AbstractMySQLPlatform::createSelectSQLBuilder()`: no SKIP LOCKED.
    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(self, Some("FOR UPDATE"), None))
    }
}
