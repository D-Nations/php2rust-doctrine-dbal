//! Port of Doctrine\DBAL\Platforms\PostgreSQLPlatform.php: the
//! `PostgreSQL` dialect of the oldest supported version.
//!
//! `setUseBooleanTrueFalseStrings()` sets a flag that the platform keeps
//! behind `&self` (PHP objects are shared). The deprecations
//! `createReservedKeywordsList()` and `getDropIndexSQL()` (for a primary
//! key) raise when called from outside the package are not raised: the
//! trait method is the one the package calls. `createMetadataProvider()`
//! and `createSchemaManager()` are not ported (no `PostgreSQL`
//! connection); `getDefaultColumnValueSQLSnippet()`, used by the schema
//! manager, is an inherent method.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/PostgreSQLPlatform.php

use std::sync::atomic::{AtomicBool, Ordering};

use php_value::numeric::is_numeric;
use php_value::{Key, PhpArray, PhpString, Value};

use crate::TransactionIsolationLevel;
use crate::deprecations;
use crate::exception::Exception;
use crate::schema::name::UnquotedIdentifierFolding;
use crate::schema::{Column, ForeignKeyConstraint, Identifier, Index, Sequence, TableDiff};
use crate::types::{self, Types};

use super::abstract_platform::{
    AbstractPlatform, array_unique, parent, validate_create_table_options,
};
use super::date_interval_unit::DateIntervalUnit;
use super::keywords::{KeywordList, PostgreSQLKeywords};
use super::platform_state::{CreateTableOptions, PlatformState, utf8};

const DEFAULT_COLLATION: &str = "default";

const TRUE_LITERALS: [&str; 6] = ["t", "true", "y", "yes", "on", "1"];
const FALSE_LITERALS: [&str; 6] = ["f", "false", "n", "no", "off", "0"];

/// PHP's global `\UnexpectedValueException`.
pub const UNEXPECTED_VALUE_EXCEPTION: &str = "UnexpectedValueException";

/// Provides the behavior, features and SQL dialect of the `PostgreSQL`
/// database platform of the oldest supported version.
#[derive(Debug)]
pub struct PostgreSQLPlatform {
    state: PlatformState,
    use_boolean_true_false_strings: AtomicBool,
}

impl Default for PostgreSQLPlatform {
    fn default() -> Self {
        Self {
            state: PlatformState::default(),
            use_boolean_true_false_strings: AtomicBool::new(true),
        }
    }
}

fn flag(column: &PhpArray, name: &str) -> bool {
    column.get(&Key::from(name)).is_some_and(Value::is_truthy)
}

/// PHP's `trim()` with its default characters.
fn php_trim(s: &[u8]) -> &[u8] {
    let is_trimmed = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\0' | b'\x0B');
    let start = s.iter().position(|b| !is_trimmed(b)).unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !is_trimmed(b))
        .map_or(start, |i| i + 1);
    &s[start..end]
}

impl PostgreSQLPlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\PostgreSQLPlatform";

    pub fn new() -> Self {
        Self::default()
    }

    /// `PostgreSQL` has different behavior with some drivers with regard to
    /// how booleans have to be handled.
    ///
    /// Enables use of 'true'/'false' or otherwise 1 and 0 instead.
    pub fn set_use_boolean_true_false_strings(&self, flag: bool) {
        self.use_boolean_true_false_strings
            .store(flag, Ordering::Relaxed);
    }

    fn uses_boolean_true_false_strings(&self) -> bool {
        self.use_boolean_true_false_strings.load(Ordering::Relaxed)
    }

    /// `convertSingleBooleanValue()` (private): the value as a PHP bool
    /// (`None` for `null`), given to `callback`.
    fn convert_single_boolean_value(
        value: &Value,
        callback: fn(Option<bool>) -> Value,
    ) -> Result<Value, Exception> {
        match value {
            Value::Null => Ok(callback(None)),
            Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                Ok(callback(Some(value.is_truthy())))
            }
            Value::String(s) if is_numeric(s.as_bytes()) => Ok(callback(Some(value.is_truthy()))),
            Value::String(s) => {
                let literal = php_trim(s.as_bytes()).to_ascii_lowercase();
                if FALSE_LITERALS.iter().any(|l| l.as_bytes() == literal) {
                    return Ok(callback(Some(false)));
                }
                if TRUE_LITERALS.iter().any(|l| l.as_bytes() == literal) {
                    return Ok(callback(Some(true)));
                }
                let mut message = b"Unrecognized boolean literal, ".to_vec();
                message.extend_from_slice(s.as_bytes());
                message.extend_from_slice(b" given.");
                Err(Exception::new(UNEXPECTED_VALUE_EXCEPTION, message))
            }
            _ => Ok(callback(Some(true))),
        }
    }

    /// `doConvertBooleans()` (private).
    fn do_convert_booleans(
        item: &Value,
        callback: fn(Option<bool>) -> Value,
    ) -> Result<Value, Exception> {
        if let Value::Array(array) = item {
            let mut converted = array.clone();
            for (key, value) in array.iter() {
                converted.insert(
                    key.clone(),
                    Self::convert_single_boolean_value(value, callback)?,
                );
            }
            return Ok(Value::Array(converted));
        }
        Self::convert_single_boolean_value(item, callback)
    }

    /// `getTypeSQLDeclaration()` (private).
    fn get_type_sql_declaration(&self, column: &Column) -> Result<PhpString, Exception> {
        let type_ = types::get_type(column.get_type_name()).map_err(Exception::from)?;

        // SERIAL/BIGSERIAL are not "real" types and we can't alter a column to that type
        let mut column_definition = column.to_array(true)?;
        column_definition.insert("autoincrement", Value::Bool(false));

        type_.get_sql_declaration(&column_definition, self)
    }

    /// `getSequenceCacheSQL()` (private).
    fn get_sequence_cache_sql(cache_size: Option<i64>) -> String {
        match cache_size {
            Some(cache_size) if cache_size > 1 => format!(" CACHE {cache_size}"),
            _ => String::new(),
        }
    }

    /// `getDefaultColumnValueSQLSnippet()` (`@internal`, for the schema
    /// manager): the SQL that reads a column's default.
    pub fn get_default_column_value_sql_snippet(&self) -> String {
        [
            "     SELECT pg_get_expr(adbin, adrelid)",
            "     FROM pg_attrdef",
            "     WHERE c.oid = pg_attrdef.adrelid",
            "        AND pg_attrdef.adnum=a.attnum",
        ]
        .join("\n")
    }
}

impl AbstractPlatform for PostgreSQLPlatform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn platform_state(&self) -> Option<&PlatformState> {
        Some(&self.state)
    }

    /// `parent::__construct(UnquotedIdentifierFolding::LOWER)`.
    fn get_unquoted_identifier_folding(&self) -> UnquotedIdentifierFolding {
        UnquotedIdentifierFolding::Lower
    }

    fn get_regexp_expression(&self) -> Result<String, Exception> {
        Ok("SIMILAR TO".to_owned())
    }

    fn get_locate_expression(
        &self,
        string: &str,
        substring: &str,
        start: Option<&str>,
    ) -> Result<String, Exception> {
        if let Some(start) = start {
            let string = self.get_substring_expression(string, start, None);

            return Ok(format!(
                "CASE WHEN (POSITION({substring} IN {string}) = 0) THEN 0 ELSE (POSITION({substring} IN {string}) + {start} - 1) END"
            ));
        }

        Ok(format!("POSITION({substring} IN {string})"))
    }

    fn get_date_arithmetic_interval_expression(
        &self,
        date: &str,
        operator: &str,
        interval: &str,
        unit: DateIntervalUnit,
    ) -> Result<String, Exception> {
        let (interval, unit) = if unit == DateIntervalUnit::Quarter {
            (self.multiply_interval(interval, 3), DateIntervalUnit::Month)
        } else {
            (interval.to_owned(), unit)
        };

        Ok(format!(
            "({date} {operator} ({interval} || ' {}')::interval)",
            unit.value()
        ))
    }

    fn get_date_diff_expression(&self, date1: &str, date2: &str) -> Result<String, Exception> {
        Ok(format!("(DATE({date1})-DATE({date2}))"))
    }

    fn get_current_database_expression(&self) -> Result<String, Exception> {
        Ok("CURRENT_DATABASE()".to_owned())
    }

    fn supports_sequences(&self) -> bool {
        true
    }

    fn supports_schemas(&self) -> bool {
        true
    }

    fn supports_identity_columns(&self) -> bool {
        true
    }

    fn supports_partial_indexes(&self) -> bool {
        true
    }

    fn supports_comment_on_statement(&self) -> bool {
        true
    }

    fn get_list_databases_sql(&self) -> Result<String, Exception> {
        Ok("SELECT datname FROM pg_database".to_owned())
    }

    fn get_list_sequences_sql(&self, database: &str) -> Result<String, Exception> {
        Ok(format!(
            "SELECT sequence_name AS relname,
                       sequence_schema AS schemaname,
                       minimum_value AS min_value,
                       increment AS increment_by
                FROM   information_schema.sequences
                WHERE  sequence_catalog = {}
                AND    sequence_schema NOT LIKE 'pg\\_%'
                AND    sequence_schema != 'information_schema'",
            self.quote_string_literal(database)
        ))
    }

    fn get_list_views_sql(&self, _database: &str) -> Result<String, Exception> {
        Ok("SELECT quote_ident(table_name) AS viewname,
                       table_schema AS schemaname,
                       view_definition AS definition
                FROM   information_schema.views
                WHERE  view_definition IS NOT NULL"
            .to_owned())
    }

    fn get_advanced_foreign_key_options_sql(
        &self,
        foreign_key: &ForeignKeyConstraint,
    ) -> Result<String, Exception> {
        let mut query = String::new();

        if foreign_key.has_option_inner("match") {
            query.push_str(" MATCH ");
            query.push_str(&super::platform_state::concat_value(
                &foreign_key.option("match"),
            )?);
        }

        query.push_str(&parent::get_advanced_foreign_key_options_sql(
            self,
            foreign_key,
        )?);

        let deferrability_sql = self.get_constraint_deferrability_sql(foreign_key);

        if !deferrability_sql.is_empty() {
            query.push_str(&deferrability_sql);
        }

        Ok(query)
    }

    #[allow(clippy::too_many_lines, reason = "upstream's method")]
    fn get_alter_table_sql(&self, diff: &TableDiff) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        let mut comments_sql = Vec::new();

        let table = diff.get_old_table();

        let table_name_sql = table.get_quoted_name(self);

        for added_column in diff.get_added_columns() {
            let query = format!(
                "ADD {}",
                self.get_column_declaration_sql(
                    &added_column.get_quoted_name(self),
                    &added_column.to_array(true)?
                )?
            );

            sql.push(format!("ALTER TABLE {table_name_sql} {query}"));

            let comment = added_column.get_comment();

            if !comment.is_empty() {
                comments_sql.push(self.get_comment_on_column_sql(
                    &table_name_sql,
                    &added_column.get_quoted_name(self),
                    comment,
                ));
            }
        }

        for dropped_column in diff.get_dropped_columns() {
            let query = format!("DROP {}", dropped_column.get_quoted_name(self));
            sql.push(format!("ALTER TABLE {table_name_sql} {query}"));
        }

        for column_diff in diff.get_changed_columns() {
            let old_column = column_diff.get_old_column();
            let new_column = column_diff.get_new_column();

            let old_column_name = old_column.get_quoted_name(self);
            let new_column_name = new_column.get_quoted_name(self);

            if column_diff.has_name_changed() {
                sql.extend(self.get_rename_column_sql(
                    &table_name_sql,
                    &old_column_name,
                    &new_column_name,
                ));
            }

            let new_type_sql_declaration = utf8(&self.get_type_sql_declaration(new_column)?)?;
            let old_type_sql_declaration = utf8(&self.get_type_sql_declaration(old_column)?)?;

            let new_collation = new_column
                .get_collation()
                .unwrap_or_else(|| DEFAULT_COLLATION.to_owned());
            let old_collation = old_column
                .get_collation()
                .unwrap_or_else(|| DEFAULT_COLLATION.to_owned());

            let type_changed = old_type_sql_declaration != new_type_sql_declaration;
            let collation_changed = old_collation != new_collation;
            if type_changed || collation_changed {
                let mut query = format!("ALTER {new_column_name} TYPE {new_type_sql_declaration}");
                if !type_changed || new_collation != DEFAULT_COLLATION {
                    query.push(' ');
                    query.push_str(&self.get_column_collation_declaration_sql(&new_collation));
                }

                sql.push(format!("ALTER TABLE {table_name_sql} {query}"));
            }

            if column_diff.has_default_changed() {
                let default_clause = if new_column.get_default().is_null() {
                    " DROP DEFAULT".to_owned()
                } else {
                    format!(
                        " SET{}",
                        self.get_default_value_declaration_sql(&new_column.to_array(true)?)?
                    )
                };

                let query = format!("ALTER {new_column_name}{default_clause}");
                sql.push(format!("ALTER TABLE {table_name_sql} {query}"));
            }

            if column_diff.has_not_null_changed() {
                let query = format!(
                    "ALTER {new_column_name} {} NOT NULL",
                    if new_column.get_notnull() {
                        "SET"
                    } else {
                        "DROP"
                    }
                );
                sql.push(format!("ALTER TABLE {table_name_sql} {query}"));
            }

            if column_diff.has_auto_increment_changed() {
                let query = if new_column.get_autoincrement() {
                    "ADD GENERATED BY DEFAULT AS IDENTITY"
                } else {
                    "DROP IDENTITY"
                };

                sql.push(format!(
                    "ALTER TABLE {table_name_sql} ALTER {new_column_name} {query}"
                ));
            }

            if column_diff.has_comment_changed() {
                comments_sql.push(self.get_comment_on_column_sql(
                    &table_name_sql,
                    &new_column.get_quoted_name(self),
                    new_column.get_comment(),
                ));
            }
        }

        let mut result = self.get_pre_alter_table_index_foreign_key_sql(diff)?;
        result.extend(sql);
        result.extend(comments_sql);
        result.extend(self.get_post_alter_table_index_foreign_key_sql(diff)?);
        Ok(result)
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
            "ALTER INDEX {old_index_name} RENAME TO {}",
            index.get_quoted_name(self)
        )])
    }

    fn get_create_sequence_sql(&self, sequence: &Sequence) -> Result<String, Exception> {
        Ok(format!(
            "CREATE SEQUENCE {} INCREMENT BY {} MINVALUE {} START {}{}",
            sequence.get_quoted_name(self),
            sequence.get_allocation_size(),
            sequence.get_initial_value(),
            sequence.get_initial_value(),
            Self::get_sequence_cache_sql(sequence.get_cache_size())
        ))
    }

    fn get_alter_sequence_sql(&self, sequence: &Sequence) -> Result<String, Exception> {
        Ok(format!(
            "ALTER SEQUENCE {} INCREMENT BY {}{}",
            sequence.get_quoted_name(self),
            sequence.get_allocation_size(),
            Self::get_sequence_cache_sql(sequence.get_cache_size())
        ))
    }

    fn get_drop_sequence_sql(&self, name: &str) -> Result<String, Exception> {
        Ok(format!(
            "{} CASCADE",
            parent::get_drop_sequence_sql(self, name)?
        ))
    }

    fn get_drop_foreign_key_sql(
        &self,
        foreign_key: &str,
        table: &str,
    ) -> Result<String, Exception> {
        Ok(self.get_drop_constraint_sql(foreign_key, table))
    }

    fn get_drop_index_sql(&self, name: &str, table: &str) -> String {
        let primary_key_name = match table.strip_suffix('"') {
            Some(unquoted) => format!("{unquoted}_pkey\""),
            None => format!("{table}_pkey"),
        };

        if name == "\"primary\"" || name == primary_key_name {
            return self.get_drop_constraint_sql(&primary_key_name, table);
        }

        let mut name = name.to_owned();
        if let Some((schema, _)) = table.split_once('.') {
            name = format!("{schema}.{name}");
        }

        parent::get_drop_index_sql(self, &name, table)
    }

    fn get_create_table_sql_internal(
        &self,
        name: &str,
        columns: &[PhpArray],
        options: CreateTableOptions,
    ) -> Result<Vec<String>, Exception> {
        validate_create_table_options(
            &options,
            "Doctrine\\DBAL\\Platforms\\PostgreSQLPlatform::_getCreateTableSQL",
        );

        let mut query_fields = self.get_column_declaration_list_sql(columns)?;

        if let Some(primary) = options.primary.as_ref().filter(|p| !p.is_empty()) {
            query_fields.push_str(", PRIMARY KEY (");
            query_fields.push_str(&array_unique(primary).join(", "));
            query_fields.push(')');
        }

        let unlogged = if matches!(
            options.table.get(&Key::from("unlogged")),
            Some(Value::Bool(true))
        ) {
            " UNLOGGED"
        } else {
            ""
        };

        let query = format!("CREATE{unlogged} TABLE {name} ({query_fields})");

        let mut sql = vec![query];

        for index in options.indexes.iter().flatten() {
            sql.push(self.get_create_index_sql(index, name)?);
        }

        for unique_constraint in options.unique_constraints.iter().flatten() {
            sql.push(self.get_create_unique_constraint_sql(unique_constraint, name)?);
        }

        for definition in options.foreign_keys.iter().flatten() {
            sql.push(self.get_create_foreign_key_sql(definition, name)?);
        }

        Ok(sql)
    }

    fn convert_booleans(&self, item: &Value) -> Result<Value, Exception> {
        if !self.uses_boolean_true_false_strings() {
            return Ok(parent::convert_booleans(self, item));
        }

        Self::do_convert_booleans(item, |value| match value {
            None => Value::from("NULL"),
            Some(true) => Value::from("true"),
            Some(false) => Value::from("false"),
        })
    }

    fn convert_booleans_to_database_value(&self, item: &Value) -> Result<Value, Exception> {
        if !self.uses_boolean_true_false_strings() {
            // parent::convertBooleansToDatabaseValue(): $this->convertBooleans().
            return self.convert_booleans(item);
        }

        Self::do_convert_booleans(item, |value| match value {
            None => Value::Null,
            Some(value) => Value::Int(i64::from(value)),
        })
    }

    fn convert_from_boolean(&self, item: &Value) -> Option<bool> {
        if let Value::String(s) = item {
            if FALSE_LITERALS.iter().any(|l| l.as_bytes() == s.as_bytes()) {
                return Some(false);
            }
            if TRUE_LITERALS.iter().any(|l| l.as_bytes() == s.as_bytes()) {
                return Some(true);
            }
        }

        parent::convert_from_boolean(self, item)
    }

    fn get_sequence_next_val_sql(&self, sequence: &str) -> Result<String, Exception> {
        Ok(format!("SELECT NEXTVAL('{sequence}')"))
    }

    fn get_set_transaction_isolation_sql(
        &self,
        level: TransactionIsolationLevel,
    ) -> Result<String, Exception> {
        Ok(format!(
            "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL {}",
            self.get_transaction_isolation_level_sql(level)
        ))
    }

    fn get_boolean_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BOOLEAN"))
    }

    fn get_integer_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let common = self.get_common_integer_type_declaration_sql(column)?;
        Ok(PhpString::from(
            [b"INT".as_slice(), common.as_bytes()].concat(),
        ))
    }

    fn get_big_int_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let common = self.get_common_integer_type_declaration_sql(column)?;
        Ok(PhpString::from(
            [b"BIGINT".as_slice(), common.as_bytes()].concat(),
        ))
    }

    fn get_small_int_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        let common = self.get_common_integer_type_declaration_sql(column)?;
        Ok(PhpString::from(
            [b"SMALLINT".as_slice(), common.as_bytes()].concat(),
        ))
    }

    fn get_guid_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("UUID"))
    }

    fn get_date_time_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIMESTAMP(0) WITHOUT TIME ZONE"))
    }

    fn get_date_time_tz_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIMESTAMP(0) WITH TIME ZONE"))
    }

    fn get_date_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATE"))
    }

    fn get_time_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIME(0) WITHOUT TIME ZONE"))
    }

    fn get_common_integer_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        if flag(column, "autoincrement") {
            return Ok(PhpString::from(" GENERATED BY DEFAULT AS IDENTITY"));
        }

        Ok(PhpString::from(""))
    }

    fn get_varchar_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from(match length {
            Some(length) => format!("VARCHAR({length})"),
            None => "VARCHAR".to_owned(),
        }))
    }

    fn get_binary_type_declaration_sql_snippet(
        &self,
        _length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BYTEA"))
    }

    fn get_varbinary_type_declaration_sql_snippet(
        &self,
        _length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BYTEA"))
    }

    fn get_clob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TEXT"))
    }

    fn get_date_time_tz_format_string(&self) -> String {
        "Y-m-d H:i:sO".to_owned()
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

    fn get_truncate_table_sql(&self, table_name: &str, cascade: bool) -> String {
        let table_identifier = Identifier::new(table_name, false);
        let mut sql = format!("TRUNCATE {}", table_identifier.get_quoted_name(self));

        if cascade {
            sql.push_str(" CASCADE");
        }

        sql
    }

    fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("bigint", Types::BIGINT),
            ("bigserial", Types::BIGINT),
            ("bool", Types::BOOLEAN),
            ("boolean", Types::BOOLEAN),
            ("bpchar", Types::STRING),
            ("bytea", Types::BLOB),
            ("char", Types::STRING),
            ("date", Types::DATE_MUTABLE),
            ("decimal", Types::DECIMAL),
            ("double precision", Types::FLOAT),
            ("float", Types::FLOAT),
            ("float4", Types::SMALLFLOAT),
            ("float8", Types::FLOAT),
            ("inet", Types::STRING),
            ("int", Types::INTEGER),
            ("int2", Types::SMALLINT),
            ("int4", Types::INTEGER),
            ("int8", Types::BIGINT),
            ("integer", Types::INTEGER),
            ("interval", Types::STRING),
            ("json", Types::JSON),
            ("jsonb", Types::JSON),
            ("money", Types::DECIMAL),
            ("numeric", Types::DECIMAL),
            ("serial", Types::INTEGER),
            ("serial4", Types::INTEGER),
            ("serial8", Types::BIGINT),
            ("real", Types::SMALLFLOAT),
            ("smallint", Types::SMALLINT),
            ("text", Types::TEXT),
            ("time", Types::TIME_MUTABLE),
            ("timestamp", Types::DATETIME_MUTABLE),
            ("timestamptz", Types::DATETIMETZ_MUTABLE),
            ("timetz", Types::TIME_MUTABLE),
            ("tsvector", Types::TEXT),
            ("uuid", Types::GUID),
            ("varchar", Types::STRING),
            ("_varchar", Types::STRING),
        ]
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        PostgreSQLKeywords::new()
    }

    fn get_blob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BYTEA"))
    }

    fn get_default_value_declaration_sql(&self, column: &PhpArray) -> Result<String, Exception> {
        if matches!(
            column.get(&Key::from("autoincrement")),
            Some(Value::Bool(true))
        ) {
            return Ok(String::new());
        }

        parent::get_default_value_declaration_sql(self, column)
    }

    fn supports_column_collation(&self) -> bool {
        true
    }

    fn get_json_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        if flag(column, "jsonb") {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6939", || {
                "The \"jsonb\" column platform option is deprecated. Use the \"JSONB\" type instead."
                    .to_owned()
            });

            return Ok(PhpString::from("JSONB"));
        }

        Ok(PhpString::from("JSON"))
    }

    fn get_jsonb_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("JSONB"))
    }
}
