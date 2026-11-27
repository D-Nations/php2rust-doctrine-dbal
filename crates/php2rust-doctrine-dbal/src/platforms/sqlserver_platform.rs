//! Port of Doctrine\DBAL\Platforms\SQLServerPlatform.php: the Microsoft
//! SQL Server platform of the oldest supported version.
//!
//! `doModifyLimitQuery()`'s two patterns (`/^SELECT\s+DISTINCT/im` and
//! `/[\s]+order\s+by\s/im`) are matched by hand: ASCII case-insensitive,
//! PCRE's `\s` (space, `\t`, `\n`, `\v`, `\f`, `\r`), `^` at the start or
//! after a `\n`. The deprecations `createReservedKeywordsList()` and
//! `getCreatePrimaryKeySQL()` raise when called from outside the package
//! are not raised: the trait method is the one the package calls.
//! `createMetadataProvider()` and `createSchemaManager()` are not ported
//! (no SQL Server connection).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/SQLServerPlatform.php

use std::fmt::Write as _;

use php_value::numeric::is_numeric;
use php_value::{Key, PhpArray, PhpString, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::schema::name::UnquotedIdentifierFolding;
use crate::schema::{Column, ColumnDiff, Identifier, Index, Sequence, TableDiff};
use crate::sql::builder::SelectSQLBuilder;
use crate::types::Types;
use crate::{LockMode, TransactionIsolationLevel};

use super::abstract_platform::{
    AbstractPlatform, array_unique, column_flag, column_length, parent,
    validate_create_table_options,
};
use super::date_interval_unit::{DateIntervalUnit, TrimMode};
use super::keywords::{KeywordList, SQLServerKeywords};
use super::platform_state::{
    CreateTableOptions, PlatformState, concat_value, string_argument, utf8,
};
use super::sqlite_platform::GLOBAL_INVALID_ARGUMENT_EXCEPTION;
use super::sqlserver::sql::builder::SQLServerSelectSQLBuilder;

/// Provides the behavior, features and SQL dialect of the Microsoft SQL
/// Server database platform of the oldest supported version.
#[derive(Debug, Default)]
pub struct SQLServerPlatform {
    state: PlatformState,
}

/// `substr_count()` of one byte: the number of `byte`s in `haystack`.
#[allow(clippy::naive_bytecount, reason = "no bytecount dependency")]
fn bytes_count(haystack: &[u8], byte: u8) -> usize {
    haystack.iter().filter(|&&b| b == byte).count()
}

/// `$column[$name]` when set and not null (`isset()`).
fn isset<'a>(column: &'a PhpArray, name: &str) -> Option<&'a Value> {
    column.get(&Key::from(name)).filter(|v| !v.is_null())
}

/// PCRE's `\s` without the `u` modifier.
fn is_pcre_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// `[$schemaName, $tableName] = explode('.', $tableName)` when the name
/// has a dot, `dbo` otherwise.
fn split_schema(table_name: &str) -> (&str, &str) {
    match table_name.split_once('.') {
        Some((schema, rest)) => (schema, rest.split('.').next().unwrap_or_default()),
        None => ("dbo", table_name),
    }
}

impl SQLServerPlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\SQLServerPlatform";

    /// `@internal` Should be used only from within the
    /// `AbstractSchemaManager` class hierarchy.
    pub const OPTION_DEFAULT_CONSTRAINT_NAME: &'static str = "default_constraint_name";

    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an expression that converts an expression of one data type
    /// to another (private).
    fn get_convert_expression(data_type: &str, expression: &str) -> String {
        format!("CONVERT({data_type}, {expression})")
    }

    fn unquote_single_identifier(possibly_quoted_name: &str) -> &str {
        if possibly_quoted_name.starts_with('[') && possibly_quoted_name.ends_with(']') {
            // substr($name, 1, -1); '[]' and ']' ... '[' cases are 2 bytes or more.
            possibly_quoted_name
                .get(1..possibly_quoted_name.len() - 1)
                .unwrap_or_default()
        } else {
            possibly_quoted_name
        }
    }

    /// Returns the SQL statement for creating a column comment (an
    /// `MS_Description` extended property; `@internal`).
    pub fn get_create_column_comment_sql(
        &self,
        table_name: &str,
        column_name: &str,
        comment: &str,
    ) -> String {
        let (schema_name, table_name) = split_schema(table_name);

        self.get_add_extended_property_sql(
            "MS_Description",
            Some(comment),
            Some("SCHEMA"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(schema_name))),
            Some("TABLE"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(table_name))),
            Some("COLUMN"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(column_name))),
        )
    }

    /// Returns the SQL snippet for declaring a default constraint
    /// (`@internal`).
    pub fn get_default_constraint_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<String, Exception> {
        if isset(column, "default").is_none() {
            return Err(Exception::new(
                GLOBAL_INVALID_ARGUMENT_EXCEPTION,
                "Incomplete column definition. \"default\" required.",
            ));
        }

        let name = concat_value(column.get(&Key::from("name")).unwrap_or(&Value::Null))?;
        let column_name = Identifier::new(&name, false);

        Ok(format!(
            "{} FOR {}",
            self.get_default_value_declaration_sql(column)?,
            column_name.get_quoted_name(self)
        ))
    }

    /// Extend unique key constraint with required filters (private).
    fn append_unique_constraint_definition(&self, sql: &str, index: &Index) -> String {
        let fields: Vec<String> = index
            .get_quoted_columns(self)
            .into_iter()
            .map(|field| format!("{field} IS NOT NULL"))
            .collect();

        format!("{sql} WHERE {}", fields.join(" AND "))
    }

    /// Returns the SQL clause for adding a default constraint in an ALTER
    /// TABLE statement (private).
    fn get_alter_table_add_default_constraint_clause(
        &self,
        column: &Column,
    ) -> Result<String, Exception> {
        let mut column_def = column.to_array(true)?;
        column_def.insert("name", Value::string(column.get_quoted_name(self)));

        Ok(format!(
            "ADD{}",
            self.get_default_constraint_declaration_sql(&column_def)?
        ))
    }

    /// Returns the SQL clause for dropping an existing default constraint
    /// in an ALTER TABLE statement (private).
    fn get_alter_table_drop_default_constraint_clause(
        &self,
        column: &Column,
    ) -> Result<String, Exception> {
        if !column.has_platform_option(Self::OPTION_DEFAULT_CONSTRAINT_NAME) {
            return Err(Exception::new(
                GLOBAL_INVALID_ARGUMENT_EXCEPTION,
                format!(
                    "Column {} was not properly introspected as it has a default value but does not have the default constraint name.",
                    column.get_name()
                ),
            ));
        }

        let name = string_argument(
            column
                .get_platform_option(Self::OPTION_DEFAULT_CONSTRAINT_NAME)
                .unwrap_or(&Value::Null),
            "Doctrine\\DBAL\\Platforms\\SQLServerPlatform::quoteSingleIdentifier",
            1,
            "str",
        )?;

        Ok(format!(
            "DROP CONSTRAINT {}",
            self.quote_single_identifier(&name)
        ))
    }

    /// Checks whether a column alteration requires dropping its default
    /// constraint first (private).
    fn alter_column_requires_drop_default_constraint(column_diff: &ColumnDiff) -> bool {
        // We only need to drop an existing default constraint if we know the
        // column was defined with a default value before.
        if column_diff.get_old_column().get_default().is_null() {
            return false;
        }

        // We need to drop an existing default constraint if the column was
        // defined with a default value before and it has changed.
        if column_diff.has_default_changed() {
            return true;
        }

        // We need to drop an existing default constraint if the column was
        // defined with a default value before and the native column type has changed.
        column_diff.has_type_changed() || column_diff.has_fixed_changed()
    }

    /// Returns the SQL statement for altering a column comment
    /// (`@internal`).
    pub fn get_alter_column_comment_sql(
        &self,
        table_name: &str,
        column_name: &str,
        comment: &str,
    ) -> String {
        let (schema_name, table_name) = split_schema(table_name);

        self.get_update_extended_property_sql(
            "MS_Description",
            Some(comment),
            Some("SCHEMA"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(schema_name))),
            Some("TABLE"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(table_name))),
            Some("COLUMN"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(column_name))),
        )
    }

    /// Returns the SQL statement for dropping a column comment
    /// (`@internal`).
    pub fn get_drop_column_comment_sql(&self, table_name: &str, column_name: &str) -> String {
        let (schema_name, table_name) = split_schema(table_name);

        self.get_drop_extended_property_sql(
            "MS_Description",
            Some("SCHEMA"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(schema_name))),
            Some("TABLE"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(table_name))),
            Some("COLUMN"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(column_name))),
        )
    }

    /// Returns the SQL statement that will execute `sp_rename` with the
    /// given arguments (private).
    fn get_rename_sql(&self, arguments: &[&str]) -> String {
        let arguments: Vec<String> = arguments
            .iter()
            .map(|argument| self.quote_national_string_literal(argument))
            .collect();
        self.get_exec_sql("sp_rename", &arguments)
    }

    /// Returns the SQL statement for adding an extended property to a
    /// database object (`@internal`).
    #[allow(clippy::too_many_arguments, reason = "upstream's signature")]
    pub fn get_add_extended_property_sql(
        &self,
        name: &str,
        value: Option<&str>,
        level0_type: Option<&str>,
        level0_name: Option<&str>,
        level1_type: Option<&str>,
        level1_name: Option<&str>,
        level2_type: Option<&str>,
        level2_name: Option<&str>,
    ) -> String {
        let mut arguments = vec![
            self.quote_national_string_literal(name),
            self.quote_national_string_literal(value.unwrap_or_default()),
            self.quote_national_string_literal(level0_type.unwrap_or_default()),
            level0_name.unwrap_or_default().to_owned(),
            self.quote_national_string_literal(level1_type.unwrap_or_default()),
            level1_name.unwrap_or_default().to_owned(),
        ];

        if level2_type.is_some() || level2_name.is_some() {
            arguments.push(self.quote_national_string_literal(level2_type.unwrap_or_default()));
            arguments.push(level2_name.unwrap_or_default().to_owned());
        }

        self.get_exec_sql("sp_addextendedproperty", &arguments)
    }

    /// Returns the SQL statement for dropping an extended property from a
    /// database object (`@internal`).
    #[allow(clippy::too_many_arguments, reason = "upstream's signature")]
    pub fn get_drop_extended_property_sql(
        &self,
        name: &str,
        level0_type: Option<&str>,
        level0_name: Option<&str>,
        level1_type: Option<&str>,
        level1_name: Option<&str>,
        level2_type: Option<&str>,
        level2_name: Option<&str>,
    ) -> String {
        let mut arguments = vec![
            self.quote_national_string_literal(name),
            self.quote_national_string_literal(level0_type.unwrap_or_default()),
            level0_name.unwrap_or_default().to_owned(),
            self.quote_national_string_literal(level1_type.unwrap_or_default()),
            level1_name.unwrap_or_default().to_owned(),
        ];

        if level2_type.is_some() || level2_name.is_some() {
            arguments.push(self.quote_national_string_literal(level2_type.unwrap_or_default()));
            arguments.push(level2_name.unwrap_or_default().to_owned());
        }

        self.get_exec_sql("sp_dropextendedproperty", &arguments)
    }

    /// Returns the SQL statement for updating an extended property of a
    /// database object (`@internal`).
    #[allow(clippy::too_many_arguments, reason = "upstream's signature")]
    pub fn get_update_extended_property_sql(
        &self,
        name: &str,
        value: Option<&str>,
        level0_type: Option<&str>,
        level0_name: Option<&str>,
        level1_type: Option<&str>,
        level1_name: Option<&str>,
        level2_type: Option<&str>,
        level2_name: Option<&str>,
    ) -> String {
        let mut arguments = vec![
            self.quote_national_string_literal(name),
            self.quote_national_string_literal(value.unwrap_or_default()),
            self.quote_national_string_literal(level0_type.unwrap_or_default()),
            level0_name.unwrap_or_default().to_owned(),
            self.quote_national_string_literal(level1_type.unwrap_or_default()),
            level1_name.unwrap_or_default().to_owned(),
        ];

        if level2_type.is_some() || level2_name.is_some() {
            arguments.push(self.quote_national_string_literal(level2_type.unwrap_or_default()));
            arguments.push(level2_name.unwrap_or_default().to_owned());
        }

        self.get_exec_sql("sp_updateextendedproperty", &arguments)
    }

    /// Returns the SQL statement that will execute the given stored
    /// procedure with the given arguments (private).
    fn get_exec_sql(&self, procedure_name: &str, arguments: &[String]) -> String {
        format!(
            "EXEC {} {}",
            self.quote_single_identifier(procedure_name),
            arguments.join(", ")
        )
    }

    fn quote_national_string_literal(&self, value: &str) -> String {
        format!("N{}", self.quote_string_literal(value))
    }

    /// `preg_match('/^SELECT\s+DISTINCT/im', $query) > 0`.
    fn is_select_distinct(query: &str) -> bool {
        let bytes = query.as_bytes();
        (0..bytes.len())
            .filter(|&i| i == 0 || bytes[i - 1] == b'\n')
            .any(|i| {
                let rest = &bytes[i..];
                if rest.len() < 6 || !rest[..6].eq_ignore_ascii_case(b"SELECT") {
                    return false;
                }
                let spaces = rest[6..].iter().take_while(|&&b| is_pcre_space(b)).count();
                spaces > 0
                    && rest[6 + spaces..]
                        .get(..8)
                        .is_some_and(|word| word.eq_ignore_ascii_case(b"DISTINCT"))
            })
    }

    /// The offsets of the matches of `/[\s]+order\s+by\s/im` in `query`.
    fn order_by_offsets(query: &str) -> Vec<usize> {
        let bytes = query.as_bytes();
        let mut offsets = Vec::new();
        let mut position = 0;
        while position < bytes.len() {
            if !is_pcre_space(bytes[position]) {
                position += 1;
                continue;
            }
            let start = position;
            let mut i = position;
            while i < bytes.len() && is_pcre_space(bytes[i]) {
                i += 1;
            }
            let matched = (|| {
                let mut j = i;
                if !bytes.get(j..j + 5)?.eq_ignore_ascii_case(b"order") {
                    return None;
                }
                j += 5;
                let spaces = bytes[j..].iter().take_while(|&&b| is_pcre_space(b)).count();
                if spaces == 0 {
                    return None;
                }
                j += spaces;
                if !bytes.get(j..j + 2)?.eq_ignore_ascii_case(b"by") {
                    return None;
                }
                j += 2;
                is_pcre_space(*bytes.get(j)?).then_some(j + 1)
            })();
            match matched {
                Some(end) => {
                    offsets.push(start);
                    position = end;
                }
                // A match attempt from a later byte of the same run of
                // spaces fails the same way.
                None => position = i,
            }
        }
        offsets
    }

    /// `shouldAddOrderBy()` (private): whether the query has no ORDER BY
    /// of its own, outside parentheses.
    fn should_add_order_by(query: &str) -> bool {
        // Find the position of the last instance of ORDER BY and ensure it is not within a parenthetical statement
        // but can be in a newline
        let offsets = Self::order_by_offsets(query);
        if offsets.is_empty() {
            return true;
        }

        // ORDER BY instance may be in a subquery after ORDER BY
        // e.g. SELECT col1 FROM test ORDER BY (SELECT col2 from test ORDER BY col2)
        // if in the searched query ORDER BY clause was found where
        // number of open parentheses after the occurrence of the clause is equal to
        // number of closed brackets after the occurrence of the clause,
        // it means that ORDER BY is included in the query being checked
        for &order_by_pos in offsets.iter().rev() {
            let tail = &query.as_bytes()[order_by_pos..];
            let open_brackets_count = bytes_count(tail, b'(');
            let closed_brackets_count = bytes_count(tail, b')');
            if open_brackets_count == closed_brackets_count {
                return false;
            }
        }

        true
    }
}

impl AbstractPlatform for SQLServerPlatform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn platform_state(&self) -> Option<&PlatformState> {
        Some(&self.state)
    }

    fn get_unquoted_identifier_folding(&self) -> UnquotedIdentifierFolding {
        UnquotedIdentifierFolding::None
    }

    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(SQLServerSelectSQLBuilder::new(self))
    }

    fn get_current_date_sql(&self) -> String {
        Self::get_convert_expression("date", "GETDATE()")
    }

    fn get_current_time_sql(&self) -> String {
        Self::get_convert_expression("time", "GETDATE()")
    }

    fn get_date_arithmetic_interval_expression(
        &self,
        date: &str,
        operator: &str,
        interval: &str,
        unit: DateIntervalUnit,
    ) -> Result<String, Exception> {
        let factor_clause = if operator == "-" { "-1 * " } else { "" };

        Ok(format!(
            "DATEADD({}, {factor_clause}{interval}, {date})",
            unit.value()
        ))
    }

    fn get_date_diff_expression(&self, date1: &str, date2: &str) -> Result<String, Exception> {
        Ok(format!("DATEDIFF(day, {date2},{date1})"))
    }

    /// Microsoft SQL Server supports this through `AUTO_INCREMENT` columns.
    fn supports_identity_columns(&self) -> bool {
        true
    }

    fn supports_release_savepoints(&self) -> bool {
        false
    }

    fn supports_schemas(&self) -> bool {
        true
    }

    fn supports_column_collation(&self) -> bool {
        true
    }

    fn supports_sequences(&self) -> bool {
        true
    }

    fn get_alter_sequence_sql(&self, sequence: &Sequence) -> Result<String, Exception> {
        Ok(format!(
            "ALTER SEQUENCE {} INCREMENT BY {}",
            sequence.get_quoted_name(self),
            sequence.get_allocation_size()
        ))
    }

    fn get_create_sequence_sql(&self, sequence: &Sequence) -> Result<String, Exception> {
        Ok(format!(
            "CREATE SEQUENCE {} START WITH {} INCREMENT BY {} MINVALUE {}",
            sequence.get_quoted_name(self),
            sequence.get_initial_value(),
            sequence.get_allocation_size(),
            sequence.get_initial_value()
        ))
    }

    fn get_list_sequences_sql(&self, _database: &str) -> Result<String, Exception> {
        Ok("SELECT seq.name,
                       CAST(
                           seq.increment AS VARCHAR(MAX)
                       ) AS increment, -- CAST avoids driver error for sql_variant type
                       CAST(
                           seq.start_value AS VARCHAR(MAX)
                       ) AS start_value -- CAST avoids driver error for sql_variant type
                FROM   sys.sequences AS seq"
            .to_owned())
    }

    fn get_sequence_next_val_sql(&self, sequence: &str) -> Result<String, Exception> {
        Ok(format!("SELECT NEXT VALUE FOR {sequence}"))
    }

    fn get_drop_foreign_key_sql(
        &self,
        foreign_key: &str,
        table: &str,
    ) -> Result<String, Exception> {
        Ok(self.get_drop_constraint_sql(foreign_key, table))
    }

    fn get_drop_index_sql(&self, name: &str, table: &str) -> String {
        format!("DROP INDEX {name} ON {table}")
    }

    fn get_create_table_sql_internal(
        &self,
        name: &str,
        columns: &[PhpArray],
        options: CreateTableOptions,
    ) -> Result<Vec<String>, Exception> {
        validate_create_table_options(
            &options,
            "Doctrine\\DBAL\\Platforms\\SQLServerPlatform::_getCreateTableSQL",
        );

        let mut default_constraints_sql = Vec::new();
        let mut comments_sql = Vec::new();

        if let Some(table_comment) = isset(&options.table, "comment") {
            let table_comment = string_argument(
                table_comment,
                "Doctrine\\DBAL\\Platforms\\SQLServerPlatform::getCommentOnTableSQL",
                2,
                "comment",
            )?;
            comments_sql.push(self.get_comment_on_table_sql(name, &table_comment));
        }

        // @todo does other code breaks because of this?
        // force primary keys to be not null
        let mut columns = columns.to_vec();
        for column in &mut columns {
            if column_flag(column, "primary") {
                column.insert("notnull", Value::Bool(true));
            }

            // Build default constraints SQL statements.
            if isset(column, "default").is_some() {
                default_constraints_sql.push(format!(
                    "ALTER TABLE {name} ADD{}",
                    self.get_default_constraint_declaration_sql(column)?
                ));
            }

            let comment = isset(column, "comment")
                .cloned()
                .unwrap_or_else(|| Value::from(""));

            let numeric = match &comment {
                Value::Int(_) | Value::Float(_) => true,
                Value::String(s) => is_numeric(s.as_bytes()),
                _ => false,
            };
            if comment.is_truthy() || numeric {
                let column_name = string_argument(
                    column.get(&Key::from("name")).unwrap_or(&Value::Null),
                    "Doctrine\\DBAL\\Platforms\\SQLServerPlatform::getCreateColumnCommentSQL",
                    2,
                    "columnName",
                )?;
                let comment = string_argument(
                    &comment,
                    "Doctrine\\DBAL\\Platforms\\SQLServerPlatform::getCreateColumnCommentSQL",
                    3,
                    "comment",
                )?;
                comments_sql.push(self.get_create_column_comment_sql(name, &column_name, &comment));
            }
        }

        let mut column_list_sql = self.get_column_declaration_list_sql(&columns)?;

        for definition in options.unique_constraints.iter().flatten() {
            column_list_sql.push_str(", ");
            column_list_sql.push_str(&self.get_unique_constraint_declaration_sql(definition)?);
        }

        if let Some(primary) = options.primary.as_ref().filter(|p| !p.is_empty()) {
            let flags = if options
                .primary_index
                .as_ref()
                .is_some_and(|index| index.has_flag("nonclustered"))
            {
                " NONCLUSTERED"
            } else {
                ""
            };

            let _ = write!(
                column_list_sql,
                ", PRIMARY KEY{flags} ({})",
                array_unique(primary).join(", ")
            );
        }

        let mut query = format!("CREATE TABLE {name} ({column_list_sql}");

        let definitions: Vec<Value> = columns.iter().cloned().map(Value::Array).collect();
        let check = self.get_check_declaration_sql(&definitions)?;
        if !check.is_empty() {
            query.push_str(", ");
            query.push_str(&check);
        }

        query.push(')');

        let mut sql = vec![query];

        for index in options.indexes.iter().flatten() {
            sql.push(self.get_create_index_sql(index, name)?);
        }

        for definition in options.foreign_keys.iter().flatten() {
            sql.push(self.get_create_foreign_key_sql(definition, name)?);
        }

        sql.extend(comments_sql);
        sql.extend(default_constraints_sql);
        Ok(sql)
    }

    /// Deprecated upstream.
    fn get_create_primary_key_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        let mut sql = format!("ALTER TABLE {table} ADD PRIMARY KEY");

        if index.has_flag("nonclustered") {
            sql.push_str(" NONCLUSTERED");
        }

        Ok(format!(
            "{sql} ({})",
            index.get_quoted_columns(self).join(", ")
        ))
    }

    fn get_create_index_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        let mut constraint = parent::get_create_index_sql(self, index, table)?;

        if index.is_unique() && !index.is_primary() {
            constraint = self.append_unique_constraint_definition(&constraint, index);
        }

        Ok(constraint)
    }

    fn get_create_index_sql_flags(&self, index: &Index) -> String {
        let mut r#type = String::new();
        if index.is_unique() {
            r#type.push_str("UNIQUE ");
        }

        if index.has_flag("clustered") {
            r#type.push_str("CLUSTERED ");
        } else if index.has_flag("nonclustered") {
            r#type.push_str("NONCLUSTERED ");
        }

        r#type
    }

    #[allow(clippy::too_many_lines, reason = "upstream's method")]
    fn get_alter_table_sql(&self, diff: &TableDiff) -> Result<Vec<String>, Exception> {
        let mut query_parts = Vec::new();
        let mut sql = Vec::new();
        let mut comments_sql = Vec::new();

        let table = diff.get_old_table();

        let table_name = table.get_name();

        for column in diff.get_added_columns() {
            let column_properties = column.to_array(true)?;

            let mut add_column_sql = format!(
                "ADD {}",
                self.get_column_declaration_sql(&column.get_quoted_name(self), &column_properties)?
            );

            if isset(&column_properties, "default").is_some() {
                add_column_sql
                    .push_str(&self.get_default_value_declaration_sql(&column_properties)?);
            }

            query_parts.push(add_column_sql);

            let comment = column.get_comment();

            if !comment.is_empty() {
                comments_sql.push(self.get_create_column_comment_sql(
                    &table_name,
                    &column.get_quoted_name(self),
                    comment,
                ));
            }
        }

        for column in diff.get_dropped_columns() {
            if !column.get_default().is_null() {
                query_parts.push(self.get_alter_table_drop_default_constraint_clause(column)?);
            }

            query_parts.push(format!("DROP COLUMN {}", column.get_quoted_name(self)));
        }

        let table_name_sql = table.get_quoted_name(self);

        for column_diff in diff.get_changed_columns() {
            let new_column = column_diff.get_new_column();
            let old_column = column_diff.get_old_column();
            let name_changed = column_diff.has_name_changed();

            if name_changed {
                // sp_rename accepts the old name as a qualified name, so it should be quoted.
                let old_column_name_sql = old_column.get_quoted_name(self);

                // sp_rename accepts the new name as a literal value, so it cannot be quoted.
                let new_column_name = new_column.get_name();

                sql.extend(self.get_rename_column_sql(
                    &table_name_sql,
                    &old_column_name_sql,
                    &new_column_name,
                ));
            }

            let new_comment = new_column.get_comment();
            let has_new_comment = !new_comment.is_empty();

            let old_comment = old_column.get_comment();
            let has_old_comment = !old_comment.is_empty();

            if has_old_comment && has_new_comment && old_comment != new_comment {
                comments_sql.push(self.get_alter_column_comment_sql(
                    &table_name,
                    &new_column.get_quoted_name(self),
                    new_comment,
                ));
            } else if has_old_comment && !has_new_comment {
                comments_sql.push(
                    self.get_drop_column_comment_sql(
                        &table_name,
                        &new_column.get_quoted_name(self),
                    ),
                );
            } else if !has_old_comment && has_new_comment {
                comments_sql.push(self.get_create_column_comment_sql(
                    &table_name,
                    &new_column.get_quoted_name(self),
                    new_comment,
                ));
            }

            let column_name_sql = new_column.get_quoted_name(self);

            let new_declaration_sql =
                self.get_column_declaration_sql(&column_name_sql, &new_column.to_array(true)?)?;
            let old_declaration_sql =
                self.get_column_declaration_sql(&column_name_sql, &old_column.to_array(true)?)?;
            let declaration_sql_changed = new_declaration_sql != old_declaration_sql;

            let default_changed = column_diff.has_default_changed();

            if !declaration_sql_changed && !default_changed && !name_changed {
                continue;
            }

            let require_drop_default_constraint =
                Self::alter_column_requires_drop_default_constraint(column_diff);

            if require_drop_default_constraint {
                query_parts.push(self.get_alter_table_drop_default_constraint_clause(old_column)?);
            }

            if declaration_sql_changed {
                query_parts.push(format!("ALTER COLUMN {new_declaration_sql}"));
            }

            if !new_column.get_default().is_null()
                && (require_drop_default_constraint || default_changed)
            {
                query_parts.push(self.get_alter_table_add_default_constraint_clause(new_column)?);
            }
        }

        for query in query_parts {
            sql.push(format!("ALTER TABLE {table_name_sql} {query}"));
        }

        let mut result = self.get_pre_alter_table_index_foreign_key_sql(diff)?;
        result.extend(sql);
        result.extend(comments_sql);
        result.extend(self.get_post_alter_table_index_foreign_key_sql(diff)?);
        Ok(result)
    }

    fn get_rename_table_sql(&self, old_name: &str, new_name: &str) -> String {
        self.get_rename_sql(&[old_name, new_name])
    }

    fn get_rename_index_sql(
        &self,
        old_index_name: &str,
        index: &Index,
        table_name: &str,
    ) -> Result<Vec<String>, Exception> {
        Ok(vec![self.get_rename_sql(&[
            &format!("{table_name}.{old_index_name}"),
            &index.get_name(),
            "INDEX",
        ])])
    }

    fn get_rename_column_sql(
        &self,
        table_name: &str,
        old_column_name: &str,
        new_column_name: &str,
    ) -> Vec<String> {
        vec![self.get_rename_sql(&[&format!("{table_name}.{old_column_name}"), new_column_name])]
    }

    fn get_empty_identity_insert_sql(
        &self,
        quoted_table_name: &str,
        _quoted_identifier_column_name: &str,
    ) -> String {
        format!("INSERT INTO {quoted_table_name} DEFAULT VALUES")
    }

    fn get_list_views_sql(&self, _database: &str) -> Result<String, Exception> {
        Ok("SELECT name, definition FROM sysobjects
                    INNER JOIN sys.sql_modules ON sysobjects.id = sys.sql_modules.object_id
                WHERE type = 'V' ORDER BY name"
            .to_owned())
    }

    fn get_locate_expression(
        &self,
        string: &str,
        substring: &str,
        start: Option<&str>,
    ) -> Result<String, Exception> {
        Ok(match start {
            None => format!("CHARINDEX({substring}, {string})"),
            Some(start) => format!("CHARINDEX({substring}, {string}, {start})"),
        })
    }

    fn get_mod_expression(&self, dividend: &str, divisor: &str) -> String {
        format!("{dividend} % {divisor}")
    }

    fn get_trim_expression(&self, str: &str, mode: TrimMode, char: Option<&str>) -> String {
        let Some(char) = char else {
            return match mode {
                TrimMode::Leading => format!("LTRIM({str})"),
                TrimMode::Trailing => format!("RTRIM({str})"),
                _ => format!("LTRIM(RTRIM({str}))"),
            };
        };

        let pattern = format!("'%[^' + {char} + ']%'");

        if mode == TrimMode::Leading {
            return format!("stuff({str}, 1, patindex({pattern}, {str}) - 1, null)");
        }

        if mode == TrimMode::Trailing {
            return format!(
                "reverse(stuff(reverse({str}), 1, patindex({pattern}, reverse({str})) - 1, null))"
            );
        }

        format!(
            "reverse(stuff(reverse(stuff({str}, 1, patindex({pattern}, {str}) - 1, null)), 1, \
             patindex({pattern}, reverse(stuff({str}, 1, patindex({pattern}, {str}) - 1, null))) - 1, null))"
        )
    }

    fn get_concat_expression(&self, strings: &[&str]) -> String {
        format!("CONCAT({})", strings.join(", "))
    }

    fn get_list_databases_sql(&self) -> Result<String, Exception> {
        Ok("SELECT * FROM sys.databases".to_owned())
    }

    fn get_substring_expression(&self, string: &str, start: &str, length: Option<&str>) -> String {
        match length {
            None => format!("SUBSTRING({string}, {start}, LEN({string}) - {start} + 1)"),
            Some(length) => format!("SUBSTRING({string}, {start}, {length})"),
        }
    }

    fn get_length_expression(&self, string: &str) -> String {
        format!("LEN({string})")
    }

    fn get_current_database_expression(&self) -> Result<String, Exception> {
        Ok("DB_NAME()".to_owned())
    }

    fn get_set_transaction_isolation_sql(
        &self,
        level: TransactionIsolationLevel,
    ) -> Result<String, Exception> {
        Ok(format!(
            "SET TRANSACTION ISOLATION LEVEL {}",
            self.get_transaction_isolation_level_sql(level)
        ))
    }

    fn get_integer_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let common = self.get_common_integer_type_declaration_sql(column)?;
        Ok(PhpString::from(format!("INT{}", utf8(&common)?)))
    }

    fn get_big_int_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let common = self.get_common_integer_type_declaration_sql(column)?;
        Ok(PhpString::from(format!("BIGINT{}", utf8(&common)?)))
    }

    fn get_small_int_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        let common = self.get_common_integer_type_declaration_sql(column)?;
        Ok(PhpString::from(format!("SMALLINT{}", utf8(&common)?)))
    }

    fn get_guid_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("UNIQUEIDENTIFIER"))
    }

    fn get_date_time_tz_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATETIMEOFFSET(6)"))
    }

    fn get_char_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from(match length {
            Some(length) => format!("NCHAR({length})"),
            None => "NCHAR".to_owned(),
        }))
    }

    fn get_varchar_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        match length {
            None => Err(Exception::column_length_required(self.class(), "NVARCHAR")),
            Some(length) => Ok(PhpString::from(format!("NVARCHAR({length})"))),
        }
    }

    fn get_ascii_string_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        let length = column_length(
            column,
            "getVarcharTypeDeclarationSQLSnippet",
            "getCharTypeDeclarationSQLSnippet",
        )?;

        if !column_flag(column, "fixed") {
            return parent::get_varchar_type_declaration_sql_snippet(self, length);
        }

        Ok(parent::get_char_type_declaration_sql_snippet(self, length))
    }

    fn get_clob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("VARCHAR(MAX)"))
    }

    fn get_common_integer_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        if column_flag(column, "autoincrement") {
            return Ok(PhpString::from(" IDENTITY"));
        }

        Ok(PhpString::from(""))
    }

    fn get_date_time_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        // 3 - microseconds precision length
        // http://msdn.microsoft.com/en-us/library/ms187819.aspx
        Ok(PhpString::from("DATETIME2(6)"))
    }

    fn get_date_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATE"))
    }

    fn get_time_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIME(0)"))
    }

    fn get_boolean_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BIT"))
    }

    fn do_modify_limit_query(&self, query: &str, limit: Option<i64>, offset: i64) -> String {
        if limit.is_none() && offset <= 0 {
            return query.to_owned();
        }

        let mut query = query.to_owned();

        if Self::should_add_order_by(&query) {
            if Self::is_select_distinct(&query) {
                // SQL Server won't let us order by a non-selected column in a DISTINCT query,
                // so we have to do this madness. This says, order by the first column in the
                // result. SQL Server's docs say that a nonordered query's result order is non-
                // deterministic anyway, so this won't do anything that a bunch of update and
                // deletes to the table wouldn't do anyway.
                query.push_str(" ORDER BY 1");
            } else {
                // In another DBMS, we could do ORDER BY 0, but SQL Server gets angry if you
                // use constant expressions in the order by list.
                query.push_str(" ORDER BY (SELECT 0)");
            }
        }

        // This looks somewhat like MYSQL, but limit/offset are in inverse positions
        // Supposedly SQL:2008 core standard.
        // Per TSQL spec, FETCH NEXT n ROWS ONLY is not valid without OFFSET n ROWS.
        let _ = write!(query, " OFFSET {offset} ROWS");

        if let Some(limit) = limit {
            let _ = write!(query, " FETCH NEXT {limit} ROWS ONLY");
        }

        query
    }

    /// `(int) (bool) $value` for a boolean or a numeric value (in an
    /// array, one level deep).
    fn convert_booleans(&self, item: &Value) -> Result<Value, Exception> {
        fn convertible(value: &Value) -> bool {
            match value {
                Value::Bool(_) | Value::Int(_) | Value::Float(_) => true,
                Value::String(s) => is_numeric(s.as_bytes()),
                _ => false,
            }
        }

        Ok(match item {
            Value::Array(array) => {
                let mut converted = array.clone();
                for (key, value) in array.iter() {
                    if convertible(value) {
                        converted.insert(key.clone(), Value::Int(i64::from(value.is_truthy())));
                    }
                }
                Value::Array(converted)
            }
            other if convertible(other) => Value::Int(i64::from(other.is_truthy())),
            other => other.clone(),
        })
    }

    fn get_create_temporary_table_snippet_sql(&self) -> String {
        "CREATE TABLE".to_owned()
    }

    fn get_temporary_table_name(&self, table_name: &str) -> String {
        format!("#{table_name}")
    }

    fn get_date_time_format_string(&self) -> String {
        "Y-m-d H:i:s.u".to_owned()
    }

    fn get_date_format_string(&self) -> String {
        "Y-m-d".to_owned()
    }

    fn get_time_format_string(&self) -> String {
        "H:i:s".to_owned()
    }

    fn get_date_time_tz_format_string(&self) -> String {
        "Y-m-d H:i:s.u P".to_owned()
    }

    fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("bigint", Types::BIGINT),
            ("binary", Types::BINARY),
            ("bit", Types::BOOLEAN),
            ("blob", Types::BLOB),
            ("char", Types::STRING),
            ("date", Types::DATE_MUTABLE),
            ("datetime", Types::DATETIME_MUTABLE),
            ("datetime2", Types::DATETIME_MUTABLE),
            ("datetimeoffset", Types::DATETIMETZ_MUTABLE),
            ("decimal", Types::DECIMAL),
            ("double", Types::FLOAT),
            ("double precision", Types::FLOAT),
            ("float", Types::FLOAT),
            ("image", Types::BLOB),
            ("int", Types::INTEGER),
            ("money", Types::INTEGER),
            ("nchar", Types::STRING),
            ("ntext", Types::TEXT),
            ("numeric", Types::DECIMAL),
            ("nvarchar", Types::STRING),
            ("real", Types::SMALLFLOAT),
            ("smalldatetime", Types::DATETIME_MUTABLE),
            ("smallint", Types::SMALLINT),
            ("smallmoney", Types::INTEGER),
            ("sysname", Types::STRING),
            ("text", Types::TEXT),
            ("time", Types::TIME_MUTABLE),
            ("tinyint", Types::SMALLINT),
            ("uniqueidentifier", Types::GUID),
            ("varbinary", Types::BINARY),
            ("varchar", Types::STRING),
            ("xml", Types::TEXT),
        ]
    }

    fn create_save_point(&self, savepoint: &str) -> String {
        format!("SAVE TRANSACTION {savepoint}")
    }

    fn release_save_point(&self, _savepoint: &str) -> String {
        String::new()
    }

    fn rollback_save_point(&self, savepoint: &str) -> String {
        format!("ROLLBACK TRANSACTION {savepoint}")
    }

    fn get_foreign_key_referential_action_sql(&self, action: &str) -> Result<String, Exception> {
        // RESTRICT is not supported, therefore falling back to NO ACTION.
        if action.eq_ignore_ascii_case("RESTRICT") {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6707", || {
                "Relying on automatic conversion of RESTRICT to NO ACTION for SQL Server is deprecated. Use NO ACTION explicitly instead.".to_owned()
            });

            return Ok("NO ACTION".to_owned());
        }

        parent::get_foreign_key_referential_action_sql(self, action)
    }

    fn append_lock_hint(&self, from_clause: &str, lock_mode: LockMode) -> String {
        match lock_mode {
            LockMode::None | LockMode::Optimistic => from_clause.to_owned(),
            LockMode::PessimisticRead => format!("{from_clause} WITH (HOLDLOCK, ROWLOCK)"),
            LockMode::PessimisticWrite => format!("{from_clause} WITH (UPDLOCK, ROWLOCK)"),
        }
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        SQLServerKeywords::new()
    }

    fn quote_single_identifier(&self, str: &str) -> String {
        format!("[{}]", str.replace(']', "]]"))
    }

    fn get_truncate_table_sql(&self, table_name: &str, _cascade: bool) -> String {
        let table_identifier = Identifier::new(table_name, false);

        format!("TRUNCATE TABLE {}", table_identifier.get_quoted_name(self))
    }

    fn get_blob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("VARBINARY(MAX)"))
    }

    /// No default (a default constraint), charset or comment.
    fn get_column_declaration_sql(
        &self,
        name: &str,
        column: &PhpArray,
    ) -> Result<String, Exception> {
        let declaration = if let Some(definition) = isset(column, "columnDefinition") {
            concat_value(definition)?
        } else {
            let collation = match column.get(&Key::from("collation")) {
                Some(collation) if collation.is_truthy() => format!(
                    " {}",
                    self.get_column_collation_declaration_sql(&string_argument(
                        collation,
                        "Doctrine\\DBAL\\Platforms\\SQLServerPlatform::getColumnCollationDeclarationSQL",
                        1,
                        "collation",
                    )?)
                ),
                _ => String::new(),
            };

            let notnull = if column_flag(column, "notnull") {
                " NOT NULL"
            } else {
                ""
            };

            let type_decl = utf8(
                &self
                    .get_column_type(column)?
                    .get_sql_declaration(column, self)?,
            )?;
            format!("{type_decl}{collation}{notnull}")
        };

        Ok(format!("{name} {declaration}"))
    }

    /// SQL Server does not support quoting collation identifiers.
    fn get_column_collation_declaration_sql(&self, collation: &str) -> String {
        format!("COLLATE {collation}")
    }

    fn columns_equal(&self, column1: &Column, column2: &Column) -> Result<bool, Exception> {
        if !parent::columns_equal(self, column1, column2)? {
            return Ok(false);
        }

        Ok(
            self.get_default_value_declaration_sql(&column1.to_array(true)?)?
                == self.get_default_value_declaration_sql(&column2.to_array(true)?)?,
        )
    }

    /// The `[` character is used in SQL Server's extended pattern syntax
    /// to define character ranges or sets.
    fn get_like_wildcard_characters(&self) -> String {
        format!("{}[", parent::get_like_wildcard_characters(self))
    }

    fn get_comment_on_table_sql(&self, table_name: &str, comment: &str) -> String {
        let (schema_name, table_name) = split_schema(table_name);

        self.get_add_extended_property_sql(
            "MS_Description",
            Some(comment),
            Some("SCHEMA"),
            Some(&self.quote_string_literal(schema_name)),
            Some("TABLE"),
            Some(&self.quote_string_literal(Self::unquote_single_identifier(table_name))),
            None,
            None,
        )
    }
}
