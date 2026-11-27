//! Port of Doctrine\DBAL\Platforms\AbstractMySQLPlatform.php: the base
//! implementation of the `MySQL`-like platforms.
//!
//! The PHP class is abstract; its methods are the free functions below,
//! and [`impl_abstract_mysql_platform!`] writes the trait methods that
//! call them into a concrete platform's `impl AbstractPlatform`
//! (`MySQLPlatform`, `MySQL80Platform`, `MySQL84Platform`), which adds what
//! its own PHP class overrides, `createReservedKeywordsList()` and
//! `createSelectSQLBuilder()` included.
//!
//! `getAlterTableSQL()` and `getPreAlterTableIndexForeignKeySQL()` unset
//! the indexes they handle on the `TableDiff` they are given (PHP passes
//! the object); here they work on a copy, so the caller's diff is left as
//! it was. The deprecations that `createReservedKeywordsList()` and
//! `supportsColumnLengthIndexes()` raise when called from outside the
//! package are not raised: the trait method is the one the package calls.
//! The `MySQLSchemaManager` helpers (`getColumnTypeSQLSnippet()`,
//! `fetchTableOptionsByTable()`) and `instanceof MariaDBPlatform` are the
//! [`AbstractMySQLPlatform`] trait, which every `MySQL`-like platform
//! implements; `createMetadataProvider()` is
//! `MySQLMetadataProvider::new()`. `createSchemaManager()` is not ported:
//! a connection's schema manager is `SQLite`'s (see
//! `AbstractPlatform::create_schema_manager()`), and a `MySQLSchemaManager`
//! is made with `MySQLSchemaManager::new()`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/AbstractMySQLPlatform.php

use std::fmt::Write as _;

use php_value::numeric::{Number, is_numeric, numeric_value};
use php_value::{Key, PhpArray, PhpString, Value};

use crate::TransactionIsolationLevel;
use crate::deprecations;
use crate::exception::Exception;
use crate::schema::{ForeignKeyConstraint, Index, TableDiff};
use crate::types::Types;

use super::abstract_platform::{
    AbstractPlatform, array_unique, parent, validate_create_table_options,
};
use super::platform_state::{CreateTableOptions, concat_value};

pub const LENGTH_LIMIT_TINYTEXT: i64 = 255;
pub const LENGTH_LIMIT_TEXT: i64 = 65535;
pub const LENGTH_LIMIT_MEDIUMTEXT: i64 = 16_777_215;

pub const LENGTH_LIMIT_TINYBLOB: i64 = 255;
pub const LENGTH_LIMIT_BLOB: i64 = 65535;
pub const LENGTH_LIMIT_MEDIUMBLOB: i64 = 16_777_215;

/// `! empty($column['length']) && is_numeric($column['length'])`: the
/// length as a number to compare.
fn numeric_length(column: &PhpArray) -> Option<f64> {
    #[allow(clippy::cast_precision_loss, reason = "compared with small limits")]
    match column.get(&Key::from("length")) {
        Some(Value::Int(i)) if *i != 0 => Some(*i as f64),
        Some(Value::Float(f)) if *f != 0.0 => Some(*f),
        Some(Value::String(s))
            if !s.is_empty() && s.as_bytes() != b"0" && is_numeric(s.as_bytes()) =>
        {
            match numeric_value(s.as_bytes()) {
                Some(Number::Int(i)) => Some(i as f64),
                Some(Number::Float(f)) => Some(f),
                None => None,
            }
        }
        _ => None,
    }
}

fn flag(column: &PhpArray, name: &str) -> bool {
    column.get(&Key::from(name)).is_some_and(Value::is_truthy)
}

/// `isset($options[$name])`: the option, when set and not null.
fn isset<'a>(options: &'a PhpArray, name: &str) -> Option<&'a Value> {
    options.get(&Key::from(name)).filter(|v| !v.is_null())
}

/// `doModifyLimitQuery()`.
pub(crate) fn do_modify_limit_query(query: &str, limit: Option<i64>, offset: i64) -> String {
    let mut query = query.to_owned();
    if let Some(limit) = limit {
        let _ = write!(query, " LIMIT {limit}");
        if offset > 0 {
            let _ = write!(query, " OFFSET {offset}");
        }
    } else if offset > 0 {
        // 2^64-1 is the maximum of unsigned BIGINT, the biggest limit possible
        let _ = write!(query, " LIMIT 18446744073709551615 OFFSET {offset}");
    }
    query
}

/// `quoteSingleIdentifier()`.
pub(crate) fn quote_single_identifier(str: &str) -> String {
    format!("`{}`", str.replace('`', "``"))
}

/// `getLocateExpression()`.
pub(crate) fn get_locate_expression(string: &str, substring: &str, start: Option<&str>) -> String {
    match start {
        None => format!("LOCATE({substring}, {string})"),
        Some(start) => format!("LOCATE({substring}, {string}, {start})"),
    }
}

/// `getConcatExpression()`.
pub(crate) fn get_concat_expression(strings: &[&str]) -> String {
    format!("CONCAT({})", strings.join(", "))
}

/// `getListViewsSQL()`.
pub(crate) fn get_list_views_sql<P: AbstractPlatform + ?Sized>(this: &P, database: &str) -> String {
    format!(
        "SELECT * FROM information_schema.VIEWS WHERE TABLE_SCHEMA = {}",
        this.quote_string_literal(database)
    )
}

/// `getJsonTypeDeclarationSQL()`.
pub(crate) fn get_json_type_declaration_sql(column: &PhpArray) -> PhpString {
    if flag(column, "jsonb") {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6939", || {
            "The \"jsonb\" column platform option is deprecated. Use the \"JSONB\" type instead."
                .to_owned()
        });
    }
    PhpString::from("JSON")
}

/// `getClobTypeDeclarationSQL()`: TINYTEXT, TEXT, MEDIUMTEXT or LONGTEXT
/// by the length.
#[allow(clippy::cast_precision_loss, reason = "the limits are exact in f64")]
pub(crate) fn get_clob_type_declaration_sql(column: &PhpArray) -> PhpString {
    if let Some(length) = numeric_length(column) {
        if length <= LENGTH_LIMIT_TINYTEXT as f64 {
            return PhpString::from("TINYTEXT");
        }
        if length <= LENGTH_LIMIT_TEXT as f64 {
            return PhpString::from("TEXT");
        }
        if length <= LENGTH_LIMIT_MEDIUMTEXT as f64 {
            return PhpString::from("MEDIUMTEXT");
        }
    }
    PhpString::from("LONGTEXT")
}

/// `getBlobTypeDeclarationSQL()`: TINYBLOB, BLOB, MEDIUMBLOB or LONGBLOB
/// by the length.
#[allow(clippy::cast_precision_loss, reason = "the limits are exact in f64")]
pub(crate) fn get_blob_type_declaration_sql(column: &PhpArray) -> PhpString {
    if let Some(length) = numeric_length(column) {
        if length <= LENGTH_LIMIT_TINYBLOB as f64 {
            return PhpString::from("TINYBLOB");
        }
        if length <= LENGTH_LIMIT_BLOB as f64 {
            return PhpString::from("BLOB");
        }
        if length <= LENGTH_LIMIT_MEDIUMBLOB as f64 {
            return PhpString::from("MEDIUMBLOB");
        }
    }
    PhpString::from("LONGBLOB")
}

/// `getDateTimeTypeDeclarationSQL()`.
pub(crate) fn get_date_time_type_declaration_sql(column: &PhpArray) -> PhpString {
    if matches!(column.get(&Key::from("version")), Some(Value::Bool(true))) {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6940", || {
            "The \"version\" column platform option is deprecated.".to_owned()
        });
        return PhpString::from("TIMESTAMP");
    }
    PhpString::from("DATETIME")
}

/// `_getCreateTableSQL()`.
pub(crate) fn get_create_table_sql_internal<P: AbstractPlatform + ?Sized>(
    this: &P,
    name: &str,
    columns: &[PhpArray],
    options: &CreateTableOptions,
) -> Result<Vec<String>, Exception> {
    validate_create_table_options(
        options,
        "Doctrine\\DBAL\\Platforms\\AbstractMySQLPlatform::_getCreateTableSQL",
    );

    let mut query_fields = this.get_column_declaration_list_sql(columns)?;

    for definition in options.unique_constraints.iter().flatten() {
        query_fields.push_str(", ");
        query_fields.push_str(&this.get_unique_constraint_declaration_sql(definition)?);
    }

    // add all indexes
    for definition in options.indexes.iter().flatten() {
        query_fields.push_str(", ");
        query_fields.push_str(&this.get_index_declaration_sql(definition)?);
    }

    // attach all primary keys
    if let Some(primary) = options.primary.as_ref().filter(|p| !p.is_empty()) {
        let _ = write!(
            query_fields,
            ", PRIMARY KEY ({})",
            array_unique(primary).join(", ")
        );
    }

    let mut sql = vec!["CREATE".to_owned()];

    if flag(&options.table, "temporary") {
        sql.push("TEMPORARY".to_owned());
    }

    sql.push(format!("TABLE {name} ({query_fields})"));

    let table_options = build_table_options(this, &options.table)?;

    if !table_options.is_empty() {
        sql.push(table_options);
    }

    if let Some(partition_options) = isset(&options.table, "partition_options") {
        sql.push(concat_value(partition_options)?);
    }

    let mut sql = vec![sql.join(" ")];

    for definition in options.foreign_keys.iter().flatten() {
        sql.push(this.get_create_foreign_key_sql(definition, name)?);
    }

    Ok(sql)
}

/// `buildTableOptions()` (private): the SQL of the table options.
fn build_table_options<P: AbstractPlatform + ?Sized>(
    this: &P,
    options: &PhpArray,
) -> Result<String, Exception> {
    if let Some(table_options) = isset(options, "table_options") {
        return concat_value(table_options);
    }

    let mut table_options = Vec::new();

    if let Some(charset) = isset(options, "charset") {
        table_options.push(format!("DEFAULT CHARACTER SET {}", concat_value(charset)?));
    }

    if let Some(collation) = isset(options, "collation") {
        table_options.push(this.get_column_collation_declaration_sql(&concat_value(collation)?));
    }

    if let Some(engine) = isset(options, "engine") {
        table_options.push(format!("ENGINE = {}", concat_value(engine)?));
    }

    // Auto increment
    if let Some(auto_increment) = isset(options, "auto_increment") {
        table_options.push(format!(
            "AUTO_INCREMENT = {}",
            concat_value(auto_increment)?
        ));
    }

    // Comment
    if let Some(comment) = isset(options, "comment") {
        table_options.push(format!(
            "COMMENT = {} ",
            this.quote_string_literal(&concat_value(comment)?)
        ));
    }

    // Row format
    if let Some(row_format) = isset(options, "row_format") {
        table_options.push(format!("ROW_FORMAT = {}", concat_value(row_format)?));
    }

    Ok(table_options.join(" "))
}

/// `$column->toArray(true)` with the column's comment, as the ALTER
/// clauses declare a column.
fn column_properties(column: &crate::schema::Column) -> Result<PhpArray, Exception> {
    let mut properties = column.to_array(true)?;
    properties.insert("comment", Value::from(column.get_comment()));
    Ok(properties)
}

/// `getAlterTableSQL()`.
pub(crate) fn get_alter_table_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
) -> Result<Vec<String>, Exception> {
    let mut diff = diff.clone();
    let mut query_parts = Vec::new();

    for column in diff.get_added_columns() {
        query_parts.push(format!(
            "ADD {}",
            this.get_column_declaration_sql(
                &column.get_quoted_name(this),
                &column_properties(column)?
            )?
        ));
    }

    for column in diff.get_dropped_columns() {
        query_parts.push(format!("DROP {}", column.get_quoted_name(this)));
    }

    for column_diff in diff.get_changed_columns() {
        let new_column = column_diff.get_new_column();
        let old_column = column_diff.get_old_column();
        query_parts.push(format!(
            "CHANGE {} {}",
            old_column.get_quoted_name(this),
            this.get_column_declaration_sql(
                &new_column.get_quoted_name(this),
                &column_properties(new_column)?
            )?
        ));
    }

    let dropped_primary = index_by_lower_case_name(diff.get_dropped_indexes(), "primary");
    let added_primary = index_by_lower_case_name(diff.get_added_indexes(), "primary");

    let mut no_longer_primary_key_columns: Vec<String> = Vec::new();

    if let Some(dropped) = &dropped_primary {
        query_parts.push("DROP PRIMARY KEY".to_owned());
        no_longer_primary_key_columns = dropped.get_columns();
    }

    if let Some(added) = &added_primary {
        let key_columns = array_unique(&added.get_columns());
        query_parts.push(format!("ADD PRIMARY KEY ({})", key_columns.join(", ")));

        let added_columns = added.get_columns();
        no_longer_primary_key_columns.retain(|column| !added_columns.contains(column));

        diff.unset_added_index(added);
    }

    let mut table_sql = Vec::new();

    if let Some(dropped) = &dropped_primary {
        let old_table = diff.get_old_table();
        for column_name in &no_longer_primary_key_columns {
            if !old_table.has_column(column_name) {
                continue;
            }
            let column = old_table.get_column(column_name)?;
            if column.get_autoincrement() {
                table_sql.extend(get_pre_alter_table_alter_primary_key_sql(
                    this, &diff, dropped,
                )?);
                break;
            }
        }

        diff.unset_dropped_index(dropped);
    }

    if !query_parts.is_empty() {
        table_sql.push(format!(
            "ALTER TABLE {} {}",
            diff.get_old_table().get_quoted_name(this),
            query_parts.join(", ")
        ));
    }

    let mut sql = pre_alter_table_index_foreign_key_sql(this, &mut diff)?;
    sql.extend(table_sql);
    sql.extend(this.get_post_alter_table_index_foreign_key_sql(&diff)?);
    Ok(sql)
}

/// `getPreAlterTableIndexForeignKeySQL()` on a copy of the diff.
pub(crate) fn get_pre_alter_table_index_foreign_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
) -> Result<Vec<String>, Exception> {
    pre_alter_table_index_foreign_key_sql(this, &mut diff.clone())
}

/// `getPreAlterTableIndexForeignKeySQL()`: unsets the indexes it replaces
/// on `diff`.
fn pre_alter_table_index_foreign_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &mut TableDiff,
) -> Result<Vec<String>, Exception> {
    let mut sql = Vec::new();

    let table_name_sql = diff.get_old_table().get_quoted_name(this);

    for changed_index in diff.modified_indexes() {
        sql.extend(get_pre_alter_table_alter_primary_key_sql(
            this,
            diff,
            changed_index,
        )?);
    }

    for dropped_index in diff.get_dropped_indexes().to_vec() {
        sql.extend(get_pre_alter_table_alter_primary_key_sql(
            this,
            diff,
            &dropped_index,
        )?);

        for added_index in diff.get_added_indexes().to_vec() {
            if dropped_index.get_unquoted_columns() != added_index.get_unquoted_columns() {
                continue;
            }

            let mut index_clause = format!("INDEX {}", added_index.get_name());

            if added_index.is_primary() {
                "PRIMARY KEY".clone_into(&mut index_clause);
            } else if added_index.is_unique() {
                index_clause = format!("UNIQUE INDEX {}", added_index.get_name());
            }

            sql.push(format!(
                "ALTER TABLE {table_name_sql} DROP INDEX {}, ADD {index_clause} ({})",
                dropped_index.get_name(),
                added_index.get_quoted_columns(this).join(", ")
            ));

            diff.unset_added_index(&added_index);
            diff.unset_dropped_index(&dropped_index);

            break;
        }
    }

    sql.extend(get_pre_alter_table_alter_index_foreign_key_sql(this, diff)?);
    sql.extend(parent::get_pre_alter_table_index_foreign_key_sql(
        this, diff,
    )?);
    sql.extend(this.get_pre_alter_table_rename_index_foreign_key_sql(diff)?);
    Ok(sql)
}

/// `getPreAlterTableAlterPrimaryKeySQL()` (private).
fn get_pre_alter_table_alter_primary_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
    index: &Index,
) -> Result<Vec<String>, Exception> {
    if !index.is_primary() {
        return Ok(Vec::new());
    }

    let table = diff.get_old_table();

    let mut sql = Vec::new();

    let table_name_sql = table.get_quoted_name(this);

    // Dropping primary keys requires to unset autoincrement attribute on the particular column first.
    for column_name in index.get_columns() {
        if !table.has_column(&column_name) {
            continue;
        }

        let column = table.get_column(&column_name)?;

        if !column.get_autoincrement() {
            continue;
        }

        deprecations::trigger("https://github.com/doctrine/dbal/pull/6841", || {
            "Relying on the auto-increment attribute of a column being automatically dropped once a column is no longer part of the primary key constraint is deprecated. Instead, drop the auto-increment attribute explicitly.".to_owned()
        });

        let mut column = column.clone();
        column.set_autoincrement(false);

        sql.push(format!(
            "ALTER TABLE {table_name_sql} MODIFY {}",
            this.get_column_declaration_sql(
                &column.get_quoted_name(this),
                &column.to_array(true)?
            )?
        ));
    }

    Ok(sql)
}

/// `getPreAlterTableAlterIndexForeignKeySQL()` (private).
fn get_pre_alter_table_alter_index_foreign_key_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    diff: &TableDiff,
) -> Result<Vec<String>, Exception> {
    let table = diff.get_old_table();

    let Some(primary_key) = table.get_primary_key() else {
        return Ok(Vec::new());
    };

    let mut primary_key_columns = Vec::new();

    for column_name in primary_key.get_columns() {
        if table.has_column(&column_name) {
            primary_key_columns.push(table.get_column(&column_name)?);
        }
    }

    if primary_key_columns.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = Vec::new();

    let table_name_sql = table.get_quoted_name(this);

    for changed_index in diff.modified_indexes() {
        // Changed primary key
        if !changed_index.is_primary() {
            continue;
        }

        for column in &primary_key_columns {
            // Check if an autoincrement column was dropped from the primary key.
            if !column.get_autoincrement()
                || changed_index.get_columns().contains(&column.get_name())
            {
                continue;
            }

            // The autoincrement attribute needs to be removed from the dropped column
            // before we can drop and recreate the primary key.
            let mut column = (*column).clone();
            column.set_autoincrement(false);

            sql.push(format!(
                "ALTER TABLE {table_name_sql} MODIFY {}",
                this.get_column_declaration_sql(
                    &column.get_quoted_name(this),
                    &column.to_array(true)?
                )?
            ));
        }
    }

    Ok(sql)
}

/// `getCreateIndexSQLFlags()`.
pub(crate) fn get_create_index_sql_flags(index: &Index) -> String {
    if index.is_unique() {
        "UNIQUE ".to_owned()
    } else if index.has_flag("fulltext") {
        "FULLTEXT ".to_owned()
    } else if index.has_flag("spatial") {
        "SPATIAL ".to_owned()
    } else {
        String::new()
    }
}

/// `getUnsignedDeclaration()` (private).
fn get_unsigned_declaration(column: &PhpArray) -> &'static str {
    if flag(column, "unsigned") {
        " UNSIGNED"
    } else {
        ""
    }
}

/// `_getCommonIntegerTypeDeclarationSQL()`.
pub(crate) fn get_common_integer_type_declaration_sql(column: &PhpArray) -> String {
    let mut sql = get_unsigned_declaration(column).to_owned();
    if flag(column, "autoincrement") {
        sql.push_str(" AUTO_INCREMENT");
    }
    sql
}

/// `getFloatDeclarationSQL()`.
pub(crate) fn get_float_declaration_sql(column: &PhpArray) -> PhpString {
    PhpString::from(format!(
        "DOUBLE PRECISION{}",
        get_unsigned_declaration(column)
    ))
}

/// `getSmallFloatDeclarationSQL()`.
pub(crate) fn get_small_float_declaration_sql(column: &PhpArray) -> PhpString {
    PhpString::from(format!("FLOAT{}", get_unsigned_declaration(column)))
}

/// `getDecimalTypeDeclarationSQL()`.
pub(crate) fn get_decimal_type_declaration_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    column: &PhpArray,
) -> Result<PhpString, Exception> {
    let mut sql = parent::get_decimal_type_declaration_sql(this, column)?
        .as_bytes()
        .to_vec();
    sql.extend_from_slice(get_unsigned_declaration(column).as_bytes());
    Ok(PhpString::from(sql))
}

/// `getEnumDeclarationSQL()`.
pub(crate) fn get_enum_declaration_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    column: &PhpArray,
) -> Result<PhpString, Exception> {
    let values = match column.get(&Key::from("values")) {
        Some(Value::Array(values)) if !values.is_empty() => values,
        _ => return Err(Exception::column_values_required(this.class(), "ENUM")),
    };
    let mut quoted = Vec::new();
    for value in values.values() {
        let Value::String(value) = value else {
            return Err(Exception::new(
                Exception::TYPE_ERROR,
                format!(
                    "Doctrine\\DBAL\\Platforms\\AbstractMySQLPlatform::quoteStringLiteral(): Argument #1 ($str) must be of type string, {} given",
                    value.value_name()
                ),
            ));
        };
        let value = super::platform_state::utf8(value)?;
        quoted.push(this.quote_string_literal(&value));
    }
    Ok(PhpString::from(format!("ENUM({})", quoted.join(", "))))
}

/// `getAdvancedForeignKeyOptionsSQL()`.
pub(crate) fn get_advanced_foreign_key_options_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    foreign_key: &ForeignKeyConstraint,
) -> Result<String, Exception> {
    let mut query = String::new();
    if foreign_key.has_option_inner("match") {
        let _ = write!(
            query,
            " MATCH {}",
            concat_value(&foreign_key.option("match"))?
        );
    }
    query.push_str(&parent::get_advanced_foreign_key_options_sql(
        this,
        foreign_key,
    )?);
    Ok(query)
}

/// `getDropIndexSQL()`.
pub(crate) fn get_drop_index_sql(name: &str, table: &str) -> String {
    format!("DROP INDEX {name} ON {table}")
}

/// `getSetTransactionIsolationSQL()`.
pub(crate) fn get_set_transaction_isolation_sql<P: AbstractPlatform + ?Sized>(
    this: &P,
    level: TransactionIsolationLevel,
) -> String {
    format!(
        "SET SESSION TRANSACTION ISOLATION LEVEL {}",
        this.get_transaction_isolation_level_sql(level)
    )
}

/// `initializeDoctrineTypeMappings()`.
pub(crate) fn initialize_doctrine_type_mappings() -> Vec<(&'static str, &'static str)> {
    vec![
        ("bigint", Types::BIGINT),
        ("binary", Types::BINARY),
        ("blob", Types::BLOB),
        ("char", Types::STRING),
        ("date", Types::DATE_MUTABLE),
        ("datetime", Types::DATETIME_MUTABLE),
        ("decimal", Types::DECIMAL),
        ("double", Types::FLOAT),
        ("enum", Types::ENUM),
        ("float", Types::SMALLFLOAT),
        ("int", Types::INTEGER),
        ("integer", Types::INTEGER),
        ("json", Types::JSON),
        ("longblob", Types::BLOB),
        ("longtext", Types::TEXT),
        ("mediumblob", Types::BLOB),
        ("mediumint", Types::INTEGER),
        ("mediumtext", Types::TEXT),
        ("numeric", Types::DECIMAL),
        ("real", Types::FLOAT),
        ("set", Types::SIMPLE_ARRAY),
        ("smallint", Types::SMALLINT),
        ("string", Types::STRING),
        ("text", Types::TEXT),
        ("time", Types::TIME_MUTABLE),
        ("timestamp", Types::DATETIME_MUTABLE),
        ("tinyblob", Types::BLOB),
        ("tinyint", Types::BOOLEAN),
        ("tinytext", Types::TEXT),
        ("varbinary", Types::BINARY),
        ("varchar", Types::STRING),
        ("year", Types::DATE_MUTABLE),
    ]
}

/// `quoteStringLiteral()`: backslashes escaped as well.
pub(crate) fn quote_string_literal<P: AbstractPlatform + ?Sized>(this: &P, str: &str) -> String {
    // MySQL requires backslashes to be escaped as well.
    parent::quote_string_literal(this, &str.replace('\\', "\\\\"))
}

/// `indexIndexesByLowerCaseName($indexes)[$name]` (private).
fn index_by_lower_case_name(indexes: &[Index], name: &str) -> Option<Index> {
    // A later index of the same lower-case name replaces an earlier one.
    indexes
        .iter()
        .rev()
        .find(|index| index.get_name().to_ascii_lowercase() == name)
        .cloned()
}

/// The `AbstractPlatform` methods that `AbstractMySQLPlatform` defines,
/// except `createReservedKeywordsList()` and `createSelectSQLBuilder()`,
/// which each platform writes; `$platform` is the platform's type.
macro_rules! impl_abstract_mysql_platform {
    () => {
        /// `AbstractMySQLPlatform::createSchemaManager($connection)`.
        fn create_schema_manager<'c>(
            &'c self,
            connection: &'c $crate::Connection,
        ) -> Result<$crate::schema::SchemaManager<'c>, $crate::Exception> {
            Ok($crate::schema::MySQLSchemaManager::new(connection, self).into())
        }

        fn platform_state(&self) -> Option<&$crate::platforms::PlatformState> {
            Some(&self.state)
        }

        /// `parent::__construct(UnquotedIdentifierFolding::NONE)`.
        fn get_unquoted_identifier_folding(
            &self,
        ) -> $crate::schema::name::UnquotedIdentifierFolding {
            $crate::schema::name::UnquotedIdentifierFolding::None
        }

        fn do_modify_limit_query(&self, query: &str, limit: Option<i64>, offset: i64) -> String {
            $crate::platforms::abstract_mysql_platform::do_modify_limit_query(query, limit, offset)
        }

        fn quote_single_identifier(&self, str: &str) -> String {
            $crate::platforms::abstract_mysql_platform::quote_single_identifier(str)
        }

        fn get_regexp_expression(&self) -> Result<String, $crate::Exception> {
            Ok("RLIKE".to_owned())
        }

        fn get_locate_expression(
            &self,
            string: &str,
            substring: &str,
            start: Option<&str>,
        ) -> Result<String, $crate::Exception> {
            Ok(
                $crate::platforms::abstract_mysql_platform::get_locate_expression(
                    string, substring, start,
                ),
            )
        }

        fn get_concat_expression(&self, strings: &[&str]) -> String {
            $crate::platforms::abstract_mysql_platform::get_concat_expression(strings)
        }

        fn get_date_arithmetic_interval_expression(
            &self,
            date: &str,
            operator: &str,
            interval: &str,
            unit: $crate::platforms::DateIntervalUnit,
        ) -> Result<String, $crate::Exception> {
            let function = if operator == "+" {
                "DATE_ADD"
            } else {
                "DATE_SUB"
            };
            Ok(format!(
                "{function}({date}, INTERVAL {interval} {})",
                unit.value()
            ))
        }

        fn get_date_diff_expression(
            &self,
            date1: &str,
            date2: &str,
        ) -> Result<String, $crate::Exception> {
            Ok(format!("DATEDIFF({date1}, {date2})"))
        }

        fn get_current_database_expression(&self) -> Result<String, $crate::Exception> {
            Ok("DATABASE()".to_owned())
        }

        fn get_length_expression(&self, string: &str) -> String {
            format!("CHAR_LENGTH({string})")
        }

        fn get_list_databases_sql(&self) -> Result<String, $crate::Exception> {
            Ok("SHOW DATABASES".to_owned())
        }

        fn get_list_views_sql(&self, database: &str) -> Result<String, $crate::Exception> {
            Ok($crate::platforms::abstract_mysql_platform::get_list_views_sql(self, database))
        }

        fn get_json_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok($crate::platforms::abstract_mysql_platform::get_json_type_declaration_sql(column))
        }

        fn get_clob_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok($crate::platforms::abstract_mysql_platform::get_clob_type_declaration_sql(column))
        }

        fn get_date_time_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(
                $crate::platforms::abstract_mysql_platform::get_date_time_type_declaration_sql(
                    column,
                ),
            )
        }

        fn get_date_type_declaration_sql(
            &self,
            _column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from("DATE"))
        }

        fn get_time_type_declaration_sql(
            &self,
            _column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from("TIME"))
        }

        fn get_boolean_type_declaration_sql(
            &self,
            _column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from("TINYINT"))
        }

        /// `MySQL` supports this through `AUTO_INCREMENT` columns.
        fn supports_identity_columns(&self) -> bool {
            true
        }

        fn supports_inline_column_comments(&self) -> bool {
            true
        }

        fn supports_column_collation(&self) -> bool {
            true
        }

        fn get_create_table_sql_internal(
            &self,
            name: &str,
            columns: &[php_value::PhpArray],
            options: $crate::platforms::CreateTableOptions,
        ) -> Result<Vec<String>, $crate::Exception> {
            $crate::platforms::abstract_mysql_platform::get_create_table_sql_internal(
                self, name, columns, &options,
            )
        }

        fn get_alter_table_sql(
            &self,
            diff: &$crate::schema::TableDiff,
        ) -> Result<Vec<String>, $crate::Exception> {
            $crate::platforms::abstract_mysql_platform::get_alter_table_sql(self, diff)
        }

        fn get_pre_alter_table_index_foreign_key_sql(
            &self,
            diff: &$crate::schema::TableDiff,
        ) -> Result<Vec<String>, $crate::Exception> {
            $crate::platforms::abstract_mysql_platform::get_pre_alter_table_index_foreign_key_sql(
                self, diff,
            )
        }

        fn get_create_index_sql_flags(&self, index: &$crate::schema::Index) -> String {
            $crate::platforms::abstract_mysql_platform::get_create_index_sql_flags(index)
        }

        fn get_integer_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from(format!(
                "INT{}",
                $crate::platforms::abstract_mysql_platform::get_common_integer_type_declaration_sql(
                    column
                )
            )))
        }

        fn get_big_int_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from(format!(
                "BIGINT{}",
                $crate::platforms::abstract_mysql_platform::get_common_integer_type_declaration_sql(
                    column
                )
            )))
        }

        fn get_small_int_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from(format!(
                "SMALLINT{}",
                $crate::platforms::abstract_mysql_platform::get_common_integer_type_declaration_sql(
                    column
                )
            )))
        }

        fn get_float_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok($crate::platforms::abstract_mysql_platform::get_float_declaration_sql(column))
        }

        fn get_small_float_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok($crate::platforms::abstract_mysql_platform::get_small_float_declaration_sql(column))
        }

        fn get_decimal_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            $crate::platforms::abstract_mysql_platform::get_decimal_type_declaration_sql(
                self, column,
            )
        }

        fn get_enum_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            $crate::platforms::abstract_mysql_platform::get_enum_declaration_sql(self, column)
        }

        fn get_common_integer_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok(php_value::PhpString::from(
                $crate::platforms::abstract_mysql_platform::get_common_integer_type_declaration_sql(
                    column,
                ),
            ))
        }

        fn get_column_charset_declaration_sql(&self, charset: &str) -> String {
            format!("CHARACTER SET {charset}")
        }

        fn get_advanced_foreign_key_options_sql(
            &self,
            foreign_key: &$crate::schema::ForeignKeyConstraint,
        ) -> Result<String, $crate::Exception> {
            $crate::platforms::abstract_mysql_platform::get_advanced_foreign_key_options_sql(
                self,
                foreign_key,
            )
        }

        fn get_drop_index_sql(&self, name: &str, table: &str) -> String {
            $crate::platforms::abstract_mysql_platform::get_drop_index_sql(name, table)
        }

        /// The `ALTER TABLE ... DROP CONSTRAINT` syntax is only available
        /// as of `MySQL` 8.0.19.
        fn get_drop_unique_constraint_sql(&self, name: &str, table_name: &str) -> String {
            self.get_drop_index_sql(name, table_name)
        }

        fn get_set_transaction_isolation_sql(
            &self,
            level: $crate::TransactionIsolationLevel,
        ) -> Result<String, $crate::Exception> {
            Ok(
                $crate::platforms::abstract_mysql_platform::get_set_transaction_isolation_sql(
                    self, level,
                ),
            )
        }

        fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
            $crate::platforms::abstract_mysql_platform::initialize_doctrine_type_mappings()
        }

        /// `MySQL` commits a transaction implicitly when DROP TABLE is
        /// executed, however not if DROP TEMPORARY TABLE is executed.
        fn get_drop_temporary_table_sql(&self, table: &str) -> String {
            format!("DROP TEMPORARY TABLE {table}")
        }

        fn get_blob_type_declaration_sql(
            &self,
            column: &php_value::PhpArray,
        ) -> Result<php_value::PhpString, $crate::Exception> {
            Ok($crate::platforms::abstract_mysql_platform::get_blob_type_declaration_sql(column))
        }

        fn quote_string_literal(&self, str: &str) -> String {
            $crate::platforms::abstract_mysql_platform::quote_string_literal(self, str)
        }

        fn get_default_transaction_isolation_level(&self) -> $crate::TransactionIsolationLevel {
            $crate::TransactionIsolationLevel::RepeatableRead
        }

        /// Deprecated upstream.
        fn supports_column_length_indexes(&self) -> bool {
            true
        }

        fn create_sql_parser(&self) -> $crate::sql::Parser {
            $crate::sql::Parser::new(true)
        }
    };
}

pub(crate) use impl_abstract_mysql_platform;

// Slice SchemaManager.

/// The `AbstractMySQLPlatform` methods that `MySQLSchemaManager` and
/// `MySQLMetadataProvider` call on their platform (`@internal`), and
/// `$platform instanceof MariaDBPlatform`. Every `MySQL` and `MariaDB`
/// platform implements it; `MariaDBPlatform` and its subclasses override
/// what their PHP classes do.
pub trait AbstractMySQLPlatform: AbstractPlatform {
    /// `getDoctrineTypeMapping($dbType)` through a trait object
    /// (`AbstractPlatform`'s needs `Self: Sized`).
    fn doctrine_type_mapping(&self, db_type: &str) -> Result<String, Exception>;

    /// `$this instanceof MariaDBPlatform`.
    fn is_mariadb(&self) -> bool {
        false
    }

    /// `getColumnTypeSQLSnippet($tableAlias, $databaseName)`: the SQL
    /// snippet that selects a column's data type.
    fn get_column_type_sql_snippet(&self, table_alias: &str, _database_name: &str) -> String {
        format!("{table_alias}.DATA_TYPE")
    }

    /// `fetchTableOptionsByTable($includeTableName)`: the SQL that selects
    /// the table options of a database (and table).
    fn fetch_table_options_by_table(&self, include_table_name: bool) -> String {
        fetch_table_options_by_table(
            "ccsa.COLLATION_NAME = t.TABLE_COLLATION",
            include_table_name,
        )
    }
}

/// `fetchTableOptionsByTable()` of `AbstractMySQLPlatform` (and of
/// `MariaDB1010Platform`, which joins on `FULL_COLLATION_NAME`).
pub(crate) fn fetch_table_options_by_table(
    join_condition: &str,
    include_table_name: bool,
) -> String {
    let sql = format!(
        "    SELECT t.TABLE_NAME,
           t.ENGINE,
           t.AUTO_INCREMENT,
           t.TABLE_COMMENT,
           t.CREATE_OPTIONS,
           t.TABLE_COLLATION,
           ccsa.CHARACTER_SET_NAME
      FROM information_schema.TABLES t
        INNER JOIN information_schema.COLLATION_CHARACTER_SET_APPLICABILITY ccsa
          ON {join_condition}"
    );

    let mut conditions = vec!["t.TABLE_SCHEMA = ?"];

    if include_table_name {
        conditions.push("t.TABLE_NAME = ?");
    }

    conditions.push("t.TABLE_TYPE = 'BASE TABLE'");

    format!("{sql} WHERE {}", conditions.join(" AND "))
}

/// `MariaDBPlatform::getColumnTypeSQLSnippet()`: `MariaDB` stores a
/// `JSON` column as `LONGTEXT` with a `json_valid()` check constraint,
/// which the snippet reverses.
pub(crate) fn mariadb_column_type_sql_snippet<P: AbstractPlatform + ?Sized>(
    platform: &P,
    table_alias: &str,
    database_name: &str,
) -> String {
    let sub_query_alias = format!("i_{table_alias}");

    let database_name = platform.quote_string_literal(database_name);

    // The check for `CONSTRAINT_SCHEMA = $databaseName` is mandatory here to prevent performance issues
    format!(
        "    IF(
        {table_alias}.DATA_TYPE = 'longtext'
        AND EXISTS(
            SELECT * FROM information_schema.CHECK_CONSTRAINTS {sub_query_alias}
            WHERE {sub_query_alias}.CONSTRAINT_SCHEMA = {database_name}
            AND {sub_query_alias}.TABLE_NAME = {table_alias}.TABLE_NAME
            AND {sub_query_alias}.CHECK_CLAUSE = CONCAT(
                'json_valid(`',
                    {table_alias}.COLUMN_NAME,
                '`)'
            )
        ),
        'json',
        {table_alias}.DATA_TYPE
    )"
    )
}

macro_rules! impl_mysql {
    ($platform:ty) => {
        impl AbstractMySQLPlatform for $platform {
            fn doctrine_type_mapping(&self, db_type: &str) -> Result<String, Exception> {
                self.get_doctrine_type_mapping(db_type)
            }
        }
    };
}

impl_mysql!(super::MySQLPlatform);
impl_mysql!(super::MySQL80Platform);
impl_mysql!(super::MySQL84Platform);

/// `MariaDBPlatform`'s overrides, and `MariaDB1010Platform`'s
/// `fetchTableOptionsByTable()` for it and its subclasses.
macro_rules! impl_mariadb {
    ($platform:ty) => {
        impl AbstractMySQLPlatform for $platform {
            fn doctrine_type_mapping(&self, db_type: &str) -> Result<String, Exception> {
                self.get_doctrine_type_mapping(db_type)
            }

            fn is_mariadb(&self) -> bool {
                true
            }

            fn get_column_type_sql_snippet(
                &self,
                table_alias: &str,
                database_name: &str,
            ) -> String {
                mariadb_column_type_sql_snippet(self, table_alias, database_name)
            }
        }
    };
    ($platform:ty, full_collation_name) => {
        impl AbstractMySQLPlatform for $platform {
            fn doctrine_type_mapping(&self, db_type: &str) -> Result<String, Exception> {
                self.get_doctrine_type_mapping(db_type)
            }

            fn is_mariadb(&self) -> bool {
                true
            }

            fn get_column_type_sql_snippet(
                &self,
                table_alias: &str,
                database_name: &str,
            ) -> String {
                mariadb_column_type_sql_snippet(self, table_alias, database_name)
            }

            // MariaDB-10.10.1 added FULL_COLLATION_NAME to the
            // information_schema.COLLATION_CHARACTER_SET_APPLICABILITY. A base
            // collation like uca1400_ai_ci can refer to multiple character
            // sets. The value in the information_schema.TABLES.TABLE_COLLATION
            // corresponds to the full collation name.
            fn fetch_table_options_by_table(&self, include_table_name: bool) -> String {
                fetch_table_options_by_table(
                    "ccsa.FULL_COLLATION_NAME = t.TABLE_COLLATION",
                    include_table_name,
                )
            }
        }
    };
}

impl_mariadb!(super::MariaDBPlatform);
impl_mariadb!(super::MariaDB1052Platform);
impl_mariadb!(super::MariaDB1060Platform);
impl_mariadb!(super::MariaDB1010Platform, full_collation_name);
impl_mariadb!(super::MariaDB110700Platform, full_collation_name);
impl_mariadb!(super::MariaDB120300Platform, full_collation_name);
