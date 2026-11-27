//! Port of Doctrine\DBAL\Platforms\DB2Platform.php: the Db2 database
//! platform of the oldest supported version.
//!
//! The deprecation `createReservedKeywordsList()` raises when called from
//! outside the package is not raised: the trait method is the one the
//! package calls. `createMetadataProvider()` and `createSchemaManager()`
//! are not ported (no Db2 connection).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/DB2Platform.php

use std::fmt::Write as _;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::TransactionIsolationLevel;
use crate::deprecations;
use crate::exception::Exception;
use crate::schema::name::UnquotedIdentifierFolding;
use crate::schema::{ColumnDiff, Identifier, Index, TableDiff};
use crate::sql::builder::{DefaultSelectSQLBuilder, SelectSQLBuilder};
use crate::types::{self, PhpMappingType, Types};

use super::abstract_platform::{
    AbstractPlatform, column_flag, parent, validate_create_table_options,
};
use super::date_interval_unit::DateIntervalUnit;
use super::exception::NotSupported;
use super::keywords::{DB2Keywords, KeywordList};
use super::platform_state::{CreateTableOptions, PlatformState, utf8};

/// `NotSupported::new(__METHOD__)` of a `DB2Platform` method.
fn not_supported(method: &str) -> Exception {
    NotSupported::new(&format!("Doctrine\\DBAL\\Platforms\\DB2Platform::{method}")).into()
}

/// `isset($column[$name]) && $column[$name] === true`.
fn is_true(column: &PhpArray, name: &str) -> bool {
    matches!(column.get(&Key::from(name)), Some(Value::Bool(true)))
}

/// The deprecation of the `version` column platform option.
fn trigger_version_deprecation() {
    deprecations::trigger("https://github.com/doctrine/dbal/pull/6940", || {
        "The \"version\" column platform option is deprecated.".to_owned()
    });
}

/// Provides the behavior, features and SQL dialect of the Db2 database
/// platform of the oldest supported version.
#[derive(Debug, Default)]
pub struct DB2Platform {
    state: PlatformState,
}

impl DB2Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\DB2Platform";

    pub fn new() -> Self {
        Self::default()
    }

    /// Gathers the table alteration SQL for a given column diff
    /// (`gatherAlterColumnSQL()`, private).
    fn gather_alter_column_sql(
        &self,
        table: &str,
        column_diff: &ColumnDiff,
        sql: &mut Vec<String>,
        query_parts: &mut Vec<String>,
        needs_reorg: &mut bool,
    ) -> Result<(), Exception> {
        let alter_column_clauses = self.get_alter_column_clauses_sql(column_diff, needs_reorg)?;

        if alter_column_clauses.is_empty() {
            return Ok(());
        }

        // If we have a single column alteration, we can append the clause to the main query.
        if alter_column_clauses.len() == 1 {
            query_parts.extend(alter_column_clauses);

            return Ok(());
        }

        // We have multiple alterations for the same column,
        // so we need to trigger a complete ALTER TABLE statement
        // for each ALTER COLUMN clause.
        for alter_column_clause in alter_column_clauses {
            sql.push(format!("ALTER TABLE {table} {alter_column_clause}"));
        }

        Ok(())
    }

    /// Returns the ALTER COLUMN SQL clauses for altering a column
    /// described by the given column diff (`getAlterColumnClausesSQL()`,
    /// private).
    fn get_alter_column_clauses_sql(
        &self,
        column_diff: &ColumnDiff,
        needs_reorg: &mut bool,
    ) -> Result<Vec<String>, Exception> {
        let new_column = column_diff.get_new_column();
        let column_array = new_column.to_array(true)?;

        let new_name = new_column.get_quoted_name(self);
        let old_name = column_diff.get_old_column().get_quoted_name(self);

        let alter_clause = format!("ALTER COLUMN {new_name}");

        if let Some(column_definition) = new_column.get_column_definition() {
            *needs_reorg = true;

            return Ok(vec![format!("{alter_clause} {column_definition}")]);
        }

        let mut clauses = Vec::new();

        if column_diff.has_name_changed() {
            clauses.push(format!("RENAME COLUMN {old_name} TO {new_name}"));
        }

        if column_diff.has_type_changed()
            || column_diff.has_length_changed()
            || column_diff.has_precision_changed()
            || column_diff.has_scale_changed()
            || column_diff.has_fixed_changed()
        {
            *needs_reorg = true;
            let type_ = types::get_type(new_column.get_type_name()).map_err(Exception::from)?;
            let declaration = type_.get_sql_declaration(&column_array, self)?;
            clauses.push(format!(
                "{alter_clause} SET DATA TYPE {}",
                utf8(&declaration)?
            ));
        }

        if column_diff.has_not_null_changed() {
            *needs_reorg = true;
            clauses.push(if new_column.get_notnull() {
                format!("{alter_clause} SET NOT NULL")
            } else {
                format!("{alter_clause} DROP NOT NULL")
            });
        }

        if column_diff.has_default_changed() {
            if new_column.get_default().is_null() {
                *needs_reorg = true;
                clauses.push(format!("{alter_clause} DROP DEFAULT"));
            } else {
                let default_clause = self.get_default_value_declaration_sql(&column_array)?;

                if !default_clause.is_empty() {
                    *needs_reorg = true;
                    clauses.push(format!("{alter_clause} SET{default_clause}"));
                }
            }
        }

        Ok(clauses)
    }
}

impl AbstractPlatform for DB2Platform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn platform_state(&self) -> Option<&PlatformState> {
        Some(&self.state)
    }

    /// `parent::__construct(UnquotedIdentifierFolding::UPPER)`.
    fn get_unquoted_identifier_folding(&self) -> UnquotedIdentifierFolding {
        UnquotedIdentifierFolding::Upper
    }

    fn get_blob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        // todo blob(n) with $column['length'];
        Ok(PhpString::from("BLOB(1M)"))
    }

    fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("bigint", Types::BIGINT),
            ("binary", Types::BINARY),
            ("blob", Types::BLOB),
            ("character", Types::STRING),
            ("clob", Types::TEXT),
            ("date", Types::DATE_MUTABLE),
            ("decimal", Types::DECIMAL),
            ("double", Types::FLOAT),
            ("integer", Types::INTEGER),
            ("real", Types::SMALLFLOAT),
            ("smallint", Types::SMALLINT),
            ("time", Types::TIME_MUTABLE),
            ("timestamp", Types::DATETIME_MUTABLE),
            ("varbinary", Types::BINARY),
            ("varchar", Types::STRING),
        ]
    }

    fn get_binary_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        let mut snippet = self
            .get_char_type_declaration_sql_snippet(length)?
            .as_bytes()
            .to_vec();
        snippet.extend_from_slice(b" FOR BIT DATA");
        Ok(PhpString::from(snippet))
    }

    fn get_varbinary_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        let mut snippet = self
            .get_varchar_type_declaration_sql_snippet(length)?
            .as_bytes()
            .to_vec();
        snippet.extend_from_slice(b" FOR BIT DATA");
        Ok(PhpString::from(snippet))
    }

    fn get_clob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        // todo clob(n) with $column['length'];
        Ok(PhpString::from("CLOB(1M)"))
    }

    fn get_boolean_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("SMALLINT"))
    }

    fn get_integer_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let mut sql = b"INTEGER".to_vec();
        sql.extend_from_slice(
            self.get_common_integer_type_declaration_sql(column)?
                .as_bytes(),
        );
        Ok(PhpString::from(sql))
    }

    fn get_big_int_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let mut sql = b"BIGINT".to_vec();
        sql.extend_from_slice(
            self.get_common_integer_type_declaration_sql(column)?
                .as_bytes(),
        );
        Ok(PhpString::from(sql))
    }

    fn get_small_int_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        let mut sql = b"SMALLINT".to_vec();
        sql.extend_from_slice(
            self.get_common_integer_type_declaration_sql(column)?
                .as_bytes(),
        );
        Ok(PhpString::from(sql))
    }

    fn get_common_integer_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        if column_flag(column, "autoincrement") {
            return Ok(PhpString::from(" GENERATED BY DEFAULT AS IDENTITY"));
        }

        Ok(PhpString::from(""))
    }

    fn get_bit_and_comparison_expression(&self, value1: &str, value2: &str) -> String {
        format!("BITAND({value1}, {value2})")
    }

    fn get_bit_or_comparison_expression(&self, value1: &str, value2: &str) -> String {
        format!("BITOR({value1}, {value2})")
    }

    fn get_date_arithmetic_interval_expression(
        &self,
        date: &str,
        operator: &str,
        interval: &str,
        unit: DateIntervalUnit,
    ) -> Result<String, Exception> {
        let (interval, unit) = match unit {
            DateIntervalUnit::Week => (self.multiply_interval(interval, 7), DateIntervalUnit::Day),
            DateIntervalUnit::Quarter => {
                (self.multiply_interval(interval, 3), DateIntervalUnit::Month)
            }
            _ => (interval.to_owned(), unit),
        };

        Ok(format!("{date} {operator} {interval} {}", unit.value()))
    }

    fn get_date_diff_expression(&self, date1: &str, date2: &str) -> Result<String, Exception> {
        Ok(format!("DAYS({date1}) - DAYS({date2})"))
    }

    fn get_date_time_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        if is_true(column, "version") {
            trigger_version_deprecation();

            return Ok(PhpString::from("TIMESTAMP(0) WITH DEFAULT"));
        }

        Ok(PhpString::from("TIMESTAMP(0)"))
    }

    fn get_date_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATE"))
    }

    fn get_time_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIME"))
    }

    fn get_truncate_table_sql(&self, table_name: &str, _cascade: bool) -> String {
        let table_identifier = Identifier::new(table_name, false);

        format!(
            "TRUNCATE {} IMMEDIATE",
            table_identifier.get_quoted_name(self)
        )
    }

    fn get_set_transaction_isolation_sql(
        &self,
        _level: TransactionIsolationLevel,
    ) -> Result<String, Exception> {
        Err(not_supported("getSetTransactionIsolationSQL"))
    }

    fn get_list_views_sql(&self, _database: &str) -> Result<String, Exception> {
        Ok("SELECT NAME, TEXT FROM SYSIBM.SYSVIEWS".to_owned())
    }

    fn supports_comment_on_statement(&self) -> bool {
        true
    }

    fn get_current_date_sql(&self) -> String {
        "CURRENT DATE".to_owned()
    }

    fn get_current_time_sql(&self) -> String {
        "CURRENT TIME".to_owned()
    }

    fn get_current_timestamp_sql(&self) -> String {
        "CURRENT TIMESTAMP".to_owned()
    }

    /// Index declaration in statements like CREATE TABLE is not supported.
    fn get_index_declaration_sql(&self, _index: &Index) -> Result<String, Exception> {
        Err(not_supported("getIndexDeclarationSQL"))
    }

    fn get_create_table_sql_internal(
        &self,
        name: &str,
        columns: &[PhpArray],
        options: CreateTableOptions,
    ) -> Result<Vec<String>, Exception> {
        validate_create_table_options(
            &options,
            "Doctrine\\DBAL\\Platforms\\DB2Platform::_getCreateTableSQL",
        );

        let mut options = options;
        let indexes = options.indexes.replace(Vec::new()).unwrap_or_default();

        let mut sqls = parent::get_create_table_sql_internal(self, name, columns, options)?;

        for definition in &indexes {
            sqls.push(self.get_create_index_sql(definition, name)?);
        }

        Ok(sqls)
    }

    fn get_alter_table_sql(&self, diff: &TableDiff) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        let mut comments_sql = Vec::new();

        let table_name_sql = diff.get_old_table().get_quoted_name(self);

        let mut query_parts = Vec::new();
        for column in diff.get_added_columns() {
            let column_def = column.to_array(true)?;
            let mut query_part = format!(
                "ADD COLUMN {}",
                self.get_column_declaration_sql(&column.get_quoted_name(self), &column_def)?
            );

            // Adding non-nullable columns to a table requires a default value to be specified.
            if column_flag(&column_def, "notnull")
                && column_def
                    .get(&Key::from("default"))
                    .is_none_or(Value::is_null)
                && !column_flag(&column_def, "autoincrement")
            {
                query_part.push_str(" WITH DEFAULT");
            }

            query_parts.push(query_part);

            let comment = column.get_comment();

            if !comment.is_empty() {
                comments_sql.push(self.get_comment_on_column_sql(
                    &table_name_sql,
                    &column.get_quoted_name(self),
                    comment,
                ));
            }
        }

        let mut needs_reorg = false;
        for column in diff.get_dropped_columns() {
            query_parts.push(format!("DROP COLUMN {}", column.get_quoted_name(self)));
            needs_reorg = true;
        }

        for column_diff in diff.get_changed_columns() {
            if column_diff.has_comment_changed() {
                let new_column = column_diff.get_new_column();
                comments_sql.push(self.get_comment_on_column_sql(
                    &table_name_sql,
                    &new_column.get_quoted_name(self),
                    new_column.get_comment(),
                ));
            }

            self.gather_alter_column_sql(
                &table_name_sql,
                column_diff,
                &mut sql,
                &mut query_parts,
                &mut needs_reorg,
            )?;
        }

        if !query_parts.is_empty() {
            sql.push(format!(
                "ALTER TABLE {table_name_sql} {}",
                query_parts.join(" ")
            ));
        }

        // Some table alteration operations require a table reorganization.
        if needs_reorg {
            sql.push(format!(
                "CALL SYSPROC.ADMIN_CMD ('REORG TABLE {table_name_sql}')"
            ));
        }

        let mut result = self.get_pre_alter_table_index_foreign_key_sql(diff)?;
        result.extend(sql);
        result.extend(comments_sql);
        result.extend(self.get_post_alter_table_index_foreign_key_sql(diff)?);
        Ok(result)
    }

    fn get_rename_table_sql(&self, old_name: &str, new_name: &str) -> String {
        format!("RENAME TABLE {old_name} TO {new_name}")
    }

    fn get_rename_index_sql(
        &self,
        old_index_name: &str,
        index: &Index,
        table_name: &str,
    ) -> Result<Vec<String>, Exception> {
        let mut old_index_name = old_index_name.to_owned();
        if let Some((schema, _)) = table_name.split_once('.') {
            old_index_name = format!("{schema}.{old_index_name}");
        }

        Ok(vec![format!(
            "RENAME INDEX {old_index_name} TO {}",
            index.get_quoted_name(self)
        )])
    }

    fn get_default_value_declaration_sql(&self, column: &PhpArray) -> Result<String, Exception> {
        if is_true(column, "autoincrement") {
            return Ok(String::new());
        }

        if is_true(column, "version") {
            trigger_version_deprecation();

            if self
                .get_column_type_or_null(column)?
                .is_some_and(|type_| type_.php_mapping_type() == Some(PhpMappingType::DateTime))
            {
                let mut column = column.clone();
                column.insert("default", Value::from("1"));

                return parent::get_default_value_declaration_sql(self, &column);
            }
        }

        parent::get_default_value_declaration_sql(self, column)
    }

    fn get_empty_identity_insert_sql(
        &self,
        quoted_table_name: &str,
        quoted_identifier_column_name: &str,
    ) -> String {
        format!(
            "INSERT INTO {quoted_table_name} ({quoted_identifier_column_name}) VALUES (DEFAULT)"
        )
    }

    fn get_create_temporary_table_snippet_sql(&self) -> String {
        "DECLARE GLOBAL TEMPORARY TABLE".to_owned()
    }

    fn get_temporary_table_name(&self, table_name: &str) -> String {
        format!("SESSION.{table_name}")
    }

    fn do_modify_limit_query(&self, query: &str, limit: Option<i64>, offset: i64) -> String {
        let mut query = query.to_owned();
        if offset > 0 {
            let _ = write!(query, " OFFSET {offset} ROWS");
        }

        if let Some(limit) = limit {
            let _ = write!(query, " FETCH NEXT {limit} ROWS ONLY");
        }

        query
    }

    fn get_locate_expression(
        &self,
        string: &str,
        substring: &str,
        start: Option<&str>,
    ) -> Result<String, Exception> {
        Ok(match start {
            None => format!("LOCATE({substring}, {string})"),
            Some(start) => format!("LOCATE({substring}, {string}, {start})"),
        })
    }

    fn get_substring_expression(&self, string: &str, start: &str, length: Option<&str>) -> String {
        match length {
            None => format!("SUBSTR({string}, {start})"),
            Some(length) => format!("SUBSTR({string}, {start}, {length})"),
        }
    }

    fn get_length_expression(&self, string: &str) -> String {
        format!("LENGTH({string}, CODEUNITS32)")
    }

    fn get_current_database_expression(&self) -> Result<String, Exception> {
        Ok("CURRENT_USER".to_owned())
    }

    fn supports_identity_columns(&self) -> bool {
        true
    }

    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(
            self,
            Some("WITH RR USE AND KEEP UPDATE LOCKS"),
            None,
        ))
    }

    fn get_dummy_select_sql(&self, expression: &str) -> String {
        format!("SELECT {expression} FROM sysibm.sysdummy1")
    }

    /// Db2 supports savepoints, but they work semantically different than
    /// on other vendor platforms.
    ///
    /// TODO: We have to investigate how to get Db2 up and running with
    /// savepoints.
    fn supports_savepoints(&self) -> bool {
        false
    }

    /// Deprecated upstream.
    fn create_reserved_keywords_list(&self) -> KeywordList {
        DB2Keywords::new()
    }
}
