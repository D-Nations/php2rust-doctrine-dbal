//! Port of Doctrine\DBAL\Platforms\SQLite\SQLiteMetadataProvider.php and
//! its `ForeignKeyConstraintDetails`: the `SQLite` metadata of tables,
//! columns, indexes, constraints and views, read from `sqlite_master`, the
//! `pragma_*` table-valued functions and the tables' `CREATE TABLE`
//! statements.
//!
//! The provider holds the platform as `&SQLitePlatform` and the
//! connection by reference. PHP's `assert()` calls (enabled in DBAL's
//! test suite) fail with an `AssertionError`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/SQLite/SQLiteMetadataProvider.php
//! Upstream: doctrine/dbal src/Platforms/SQLite/SQLiteMetadataProvider/ForeignKeyConstraintDetails.php

use php_value::{PhpArray, Value};

use crate::exception::Exception;
use crate::platforms::{AbstractPlatform, NotSupported, SQLitePlatform};
use crate::schema::exception::{
    sqlite_missing_foreign_key_constraint_referenced_columns,
    unsupported_name_from_non_null_schema_name,
};
use crate::schema::metadata::{
    DatabaseMetadataRow, ForeignKeyConstraintColumnMetadataRow,
    ForeignKeyConstraintColumnMetadataRowParts, IndexColumnMetadataRow, MetadataProvider,
    PrimaryKeyConstraintColumnRow, RowId, SchemaMetadataRow, SequenceMetadataRow,
    TableColumnMetadataRow, TableMetadataRow, UniqueConstraintColumnMetadataRow, ViewMetadataRow,
};
use crate::schema::{Column, IndexType, MatchType, ReferentialAction};
use crate::types::Types;
use crate::{Connection, ParameterTypes};

use super::preg::{group, preg_match, preg_match_all, preg_quote, preg_replace, rtrim_newlines};

const CLASS: &str = "Doctrine\\DBAL\\Platforms\\SQLite\\SQLiteMetadataProvider";

/// PHP's `\AssertionError` for a failed `assert()`.
pub(crate) const ASSERTION_ERROR: &str = "AssertionError";

type Rows<T> = Result<Vec<T>, Exception>;

/// Intermediate results of `SQLite` foreign key constraint introspection
/// obtained by parsing the DDL of the table that owns the foreign key
/// constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignKeyConstraintDetails {
    name: Option<String>,
    is_deferrable: bool,
    is_deferred: bool,
}

impl ForeignKeyConstraintDetails {
    pub fn new(name: Option<&str>, is_deferrable: bool, is_deferred: bool) -> Self {
        Self {
            name: name.map(str::to_owned),
            is_deferrable,
            is_deferred,
        }
    }

    pub fn get_name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn is_deferrable(&self) -> bool {
        self.is_deferrable
    }

    pub fn is_deferred(&self) -> bool {
        self.is_deferred
    }
}

/// The `SQLite` metadata provider.
#[derive(Debug, Clone, Copy)]
pub struct SQLiteMetadataProvider<'c> {
    connection: &'c Connection,
    platform: &'c SQLitePlatform,
}

/// A value of a metadata row as the string PHP's typed parameter takes.
pub(crate) fn text(value: Option<&Value>) -> Result<String, Exception> {
    match value {
        Some(Value::String(s)) => Ok(s.to_string_lossy().into_owned()),
        Some(Value::Int(i)) => Ok(i.to_string()),
        other => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "Expected a string, {} given",
                other.map_or_else(|| "null".to_owned(), Value::debug_type)
            ),
        )),
    }
}

/// A nullable string value of a metadata row.
pub(crate) fn nullable_text(value: Option<&Value>) -> Result<Option<String>, Exception> {
    match value {
        None | Some(Value::Null) => Ok(None),
        other => text(other).map(Some),
    }
}

fn cell(row: &PhpArray, index: i64) -> Option<&Value> {
    row.get(&index.into())
}

pub(crate) fn assertion_error(assertion: &str) -> Exception {
    Exception::new(ASSERTION_ERROR, format!("assert({assertion})"))
}

/// `strcasecmp($a, $b) === 0`.
fn equals_ignoring_case(a: &str, b: &str) -> bool {
    a.as_bytes().eq_ignore_ascii_case(b.as_bytes())
}

impl<'c> SQLiteMetadataProvider<'c> {
    /// `@internal` This class can be instantiated only by a database
    /// platform.
    pub fn new(connection: &'c Connection, platform: &'c SQLitePlatform) -> Self {
        Self {
            connection,
            platform,
        }
    }

    async fn fetch_all_numeric(&self, sql: &str, params: &[String]) -> Rows<PhpArray> {
        let params = PhpArray::list(params.iter().map(|p| Value::string(p.as_str())));
        self.connection
            .fetch_all_numeric(sql, &params, &ParameterTypes::new())
            .await
    }

    async fn get_table_names(&self) -> Rows<String> {
        let sql = format!(
            "SELECT name\nFROM sqlite_master\nWHERE type = 'table'\n  AND {}",
            Self::build_table_name_predicate("name")
        );
        self.connection
            .fetch_first_column(&sql, &PhpArray::new(), &ParameterTypes::new())
            .await?
            .iter()
            .map(|value| text(Some(value)))
            .collect()
    }

    /// @link <https://www.sqlite.org/pragma.html#pragma_table_info>
    async fn get_table_columns(&self, table_name: Option<&str>) -> Rows<TableColumnMetadataRow> {
        let mut params = Vec::new();

        let sql = format!(
            "SELECT t.name,\n       c.name,\n       c.type,\n       c.\"notnull\",\n       c.dflt_value\nFROM sqlite_master t\n         JOIN pragma_table_info(t.name) c\nWHERE {}\nORDER BY t.name,\n         c.cid",
            Self::build_table_query_predicate(table_name, &mut params)
        );

        let rows = self.fetch_all_numeric(&sql, &params).await?;
        let mut sql_by_table_name: Vec<(String, String)> = Vec::new();

        for row in &rows {
            let table_name = text(cell(row, 0))?;
            if !sql_by_table_name
                .iter()
                .any(|(name, _)| *name == table_name)
            {
                let sql = self.get_create_table_sql(&table_name).await?;
                sql_by_table_name.push((table_name, sql));
            }
        }

        rows.iter()
            .map(|row| self.create_table_column(row, &sql_by_table_name))
            .collect()
    }

    fn create_table_column(
        &self,
        row: &PhpArray,
        sql_by_table_name: &[(String, String)],
    ) -> Result<TableColumnMetadataRow, Exception> {
        let table_name = text(cell(row, 0))?;
        let column_name = text(cell(row, 1))?;
        let type_ = text(cell(row, 2))?;
        let not_null = cell(row, 3).is_some_and(Value::is_truthy);
        let default_expression = nullable_text(cell(row, 4))?;

        let Some(matches) = preg_match(r"/^([A-Z\s]+?)(?:\s*\((\d+)(?:,\s*(\d+))?\))?$/i", &type_)?
        else {
            return Err(assertion_error("$matchResult === 1"));
        };

        let mut editor = Column::editor().set_quoted_name(&column_name);

        let mut db_type = group(&matches, 1).unwrap_or("").to_ascii_lowercase();

        if db_type.contains(" unsigned") {
            db_type = db_type.replace(" unsigned", "");
            editor = editor.set_unsigned(true);
        }

        let type_name = self.platform.get_doctrine_type_mapping(&db_type)?;

        editor = editor.set_type_name(&type_name);

        if db_type == "char" {
            editor = editor.set_fixed(true);
        }

        if let Some(first) = group(&matches, 2) {
            if let Some(second) = group(&matches, 3) {
                editor = editor
                    .set_precision(Some(php_int(first)))
                    .set_scale(php_int(second));
            } else {
                editor = editor.set_length(Some(php_int(first)));
            }
        }

        if let Some(default_expression) = default_expression {
            editor = editor.set_default_value(
                parse_default_expression(&default_expression)?.map_or(Value::Null, Value::string),
            );
        }

        let table_sql = sql_by_table_name
            .iter()
            .find(|(name, _)| *name == table_name)
            .map_or("", |(_, sql)| sql.as_str());

        editor = editor
            .set_autoincrement(self.parse_column_auto_increment_from_sql(&column_name, table_sql)?)
            .set_comment(&self.parse_column_comment_from_sql(&column_name, table_sql)?)
            .set_not_null(not_null);

        if type_name == Types::STRING || type_name == Types::TEXT {
            let collation = self
                .parse_column_collation_from_sql(&column_name, table_sql)?
                .unwrap_or_else(|| "BINARY".to_owned());
            editor = editor.set_collation(Some(&collation));
        }

        Ok(TableColumnMetadataRow::new(
            None,
            &table_name,
            editor.create()?,
        ))
    }

    /// @link <https://www.sqlite.org/autoinc.html#the_autoincrement_keyword>
    fn parse_column_auto_increment_from_sql(
        &self,
        column: &str,
        sql: &str,
    ) -> Result<bool, Exception> {
        let pattern = format!(
            "/{}INTEGER\\s+PRIMARY\\s+KEY\\s+AUTOINCREMENT/i",
            self.build_identifier_pattern(column)
        );
        Ok(preg_match(&pattern, sql)?.is_some())
    }

    fn parse_column_collation_from_sql(
        &self,
        column: &str,
        sql: &str,
    ) -> Result<Option<String>, Exception> {
        let pattern = format!(
            "{{{}{}}}is",
            self.build_identifier_pattern(column),
            r#"[^,(]+(?:\([^()]+\)[^,]*)?(?:(?:DEFAULT|CHECK)\s*(?:\(.*?\))?[^,]*)*COLLATE\s+["']?([^\s,"')]+)"#
        );

        let Some(matches) = preg_match(&pattern, sql)? else {
            return Ok(None);
        };

        let collation = group(&matches, 1).unwrap_or("");
        if collation.is_empty() {
            return Err(assertion_error("strlen($match[1]) > 0"));
        }

        Ok(Some(collation.to_owned()))
    }

    fn parse_column_comment_from_sql(&self, column: &str, sql: &str) -> Result<String, Exception> {
        let pattern = format!(
            "{{[\\s(,]{}{}}}i",
            self.build_identifier_pattern(column),
            r"(?:\([^)]*?\)|[^,(])*?,?(\s*--[^\n]*\n?)+"
        );

        let Some(matches) = preg_match(&pattern, sql)? else {
            return Ok(String::new());
        };

        preg_replace(
            r"{^\s*--}m",
            "",
            rtrim_newlines(group(&matches, 1).unwrap_or("")),
        )
    }

    /// Returns a regular expression pattern that matches the given
    /// unquoted or quoted identifier.
    fn build_identifier_pattern(&self, identifier: &str) -> String {
        build_identifier_pattern(self.platform, identifier)
    }

    /// @link <https://www.sqlite.org/pragma.html#pragma_index_info>
    /// @link <https://www.sqlite.org/pragma.html#pragma_index_list>
    /// @link <https://www.sqlite.org/fileformat2.html#internal_schema_objects>
    async fn get_index_columns(&self, table_name: Option<&str>) -> Rows<IndexColumnMetadataRow> {
        let mut params = Vec::new();

        let sql = format!(
            "SELECT t.name,\n       i.name,\n       i.\"unique\",\n       c.name\nFROM sqlite_master t\n         JOIN pragma_index_list(t.name) i\n         JOIN pragma_index_info(i.name) c\nWHERE {}\n  AND i.name NOT LIKE 'sqlite_%'\nORDER BY t.name,\n         i.name,\n         c.seqno",
            Self::build_table_query_predicate(table_name, &mut params)
        );

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| {
                Ok(IndexColumnMetadataRow::new(
                    None,
                    &text(cell(row, 0))?,
                    &text(cell(row, 1))?,
                    if cell(row, 2).is_some_and(Value::is_truthy) {
                        IndexType::Unique
                    } else {
                        IndexType::Regular
                    },
                    false,
                    None,
                    &text(cell(row, 3))?,
                    None,
                ))
            })
            .collect()
    }

    /// @link <https://www.sqlite.org/pragma.html#pragma_table_info>
    async fn get_primary_key_constraint_columns(
        &self,
        table_name: Option<&str>,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        let mut params = Vec::new();

        let sql = format!(
            "SELECT t.name,\n       c.name\nFROM sqlite_master t\n         JOIN pragma_table_info(t.name) c\nWHERE {}\n  AND c.pk > 0\nORDER BY t.name,\n         c.pk",
            Self::build_table_query_predicate(table_name, &mut params)
        );

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| {
                Ok(PrimaryKeyConstraintColumnRow::new(
                    None,
                    &text(cell(row, 0))?,
                    None,
                    true,
                    &text(cell(row, 1))?,
                ))
            })
            .collect()
    }

    async fn get_unique_constraint_columns(
        &self,
        table_name: Option<&str>,
    ) -> Rows<UniqueConstraintColumnMetadataRow> {
        let mut params = Vec::new();

        let sql = format!(
            "SELECT t.name,\n       i.name,\n       c.name\nFROM sqlite_master t\n         JOIN pragma_index_list(t.name) i\n         JOIN pragma_index_info(i.name) c\nWHERE {}\n  AND i.origin = 'u'\nORDER BY t.name,\n         i.seq DESC,\n         c.seqno",
            Self::build_table_query_predicate(table_name, &mut params)
        );

        let mut current_table_name: Option<String> = None;
        let mut current_index_name: Option<String> = None;
        let mut constraint_names: Vec<Option<String>> = Vec::new();
        let mut position: i64 = -1;
        let mut result = Vec::new();

        for row in self.fetch_all_numeric(&sql, &params).await? {
            let table_name = text(cell(&row, 0))?;
            let index_name = text(cell(&row, 1))?;

            if current_table_name.as_deref() != Some(table_name.as_str()) {
                constraint_names = self.get_unique_constraint_names(&table_name).await?;
                current_table_name = Some(table_name.clone());
                current_index_name = None;
                position = -1;
            }

            if current_index_name.as_deref() != Some(index_name.as_str()) {
                current_index_name = Some(index_name.clone());
                position += 1;
            }

            let name = usize::try_from(position)
                .ok()
                .and_then(|position| constraint_names.get(position).cloned())
                .flatten();

            result.push(UniqueConstraintColumnMetadataRow::new(
                None,
                &table_name,
                Some(RowId::String(index_name)),
                name.as_deref(),
                &text(cell(&row, 2))?,
            )?);
        }

        Ok(result)
    }

    async fn get_unique_constraint_names(&self, table_name: &str) -> Rows<Option<String>> {
        let sql = self.get_create_table_sql(table_name).await?;

        let matches = preg_match_all(r"#(?:CONSTRAINT\s+(\S+)\s+)?UNIQUE\b#i", &sql)?;

        Ok(matches
            .get(1)
            .map(|names| {
                names
                    .iter()
                    .map(|name| parse_optionally_quoted_name(name))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// @link <https://sqlite.org/pragma.html#pragma_foreign_key_list>
    async fn get_foreign_key_constraint_columns(
        &self,
        table_name: Option<&str>,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        let mut params = Vec::new();

        let sql = format!(
            "SELECT t.name,\n       c.id,\n       c.\"table\",\n       c.on_update,\n       c.on_delete,\n       c.\"from\",\n       c.\"to\"\nFROM sqlite_master t\n         JOIN pragma_foreign_key_list(t.name) c\nWHERE {}\nORDER BY t.name,\n         c.id DESC,\n         c.seq",
            Self::build_table_query_predicate(table_name, &mut params)
        );

        let rows = self.fetch_all_numeric(&sql, &params).await?;
        self.generate_foreign_key_constraint_columns(rows).await
    }

    async fn generate_foreign_key_constraint_columns(
        &self,
        rows: Vec<PhpArray>,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        let mut current_table_name: Option<String> = None;
        let mut current_details: Vec<ForeignKeyConstraintDetails> = Vec::new();
        let mut result = Vec::new();

        for row in rows {
            let table_name = text(cell(&row, 0))?;
            let id = match cell(&row, 1) {
                Some(Value::Int(id)) => *id,
                other => text(other)?.parse().unwrap_or(0),
            };

            if current_table_name.as_deref() != Some(table_name.as_str()) {
                current_details = self.get_foreign_key_constraint_details(&table_name).await?;
                current_table_name = Some(table_name.clone());
            }

            // SQLite identifies foreign keys in reverse order of appearance in SQL
            let index = i64::try_from(current_details.len()).unwrap_or(i64::MAX) - id - 1;
            let Some(details) = usize::try_from(index)
                .ok()
                .and_then(|index| current_details.get(index))
            else {
                return Err(Exception::new(
                    "Error",
                    "Call to a member function getName() on null",
                ));
            };

            let name = details.get_name();
            let referenced_table_name = text(cell(&row, 2))?;

            let referenced_column_names = if let Some(to) = nullable_text(cell(&row, 6))? {
                vec![to]
            } else {
                // inferring a shorthand form for the foreign key constraint,
                // where the referenced column names are omitted
                let referenced_column_names: Vec<String> = self
                    .get_primary_key_constraint_columns(Some(&referenced_table_name))
                    .await?
                    .iter()
                    .map(|column| column.get_column_name().to_owned())
                    .collect();

                if referenced_column_names.is_empty() {
                    return Err(sqlite_missing_foreign_key_constraint_referenced_columns(
                        name,
                        &table_name,
                        &referenced_table_name,
                    ));
                }

                referenced_column_names
            };

            let on_update_action = create_referential_action(&text(cell(&row, 3))?)?;
            let on_delete_action = create_referential_action(&text(cell(&row, 4))?)?;
            let referencing_column_name = text(cell(&row, 5))?;

            for referenced_column_name in &referenced_column_names {
                result.push(ForeignKeyConstraintColumnMetadataRow::new(
                    ForeignKeyConstraintColumnMetadataRowParts {
                        referencing_schema_name: None,
                        referencing_table_name: &table_name,
                        id: Some(RowId::Int(id)),
                        name,
                        referenced_schema_name: None,
                        referenced_table_name: &referenced_table_name,
                        match_type: MatchType::Simple,
                        on_update_action,
                        on_delete_action,
                        is_deferrable: details.is_deferrable(),
                        is_deferred: details.is_deferred(),
                        referencing_column_name: &referencing_column_name,
                        referenced_column_name,
                    },
                )?);
            }
        }

        Ok(result)
    }

    async fn get_foreign_key_constraint_details(
        &self,
        table_name: &str,
    ) -> Rows<ForeignKeyConstraintDetails> {
        let sql = self.get_create_table_sql(table_name).await?;

        let matches = preg_match_all(FOREIGN_KEY_DETAILS_PATTERN, &sql)?;

        if matches.first().is_none_or(Vec::is_empty) {
            return Ok(Vec::new());
        }

        let names = &matches[1];
        let deferrable = &matches[2];
        let deferred = &matches[3];

        Ok((0..matches[0].len())
            .map(|i| {
                ForeignKeyConstraintDetails::new(
                    parse_optionally_quoted_name(&names[i]).as_deref(),
                    equals_ignoring_case(&deferrable[i], "deferrable"),
                    equals_ignoring_case(&deferred[i], "deferred"),
                )
            })
            .collect())
    }

    async fn get_create_table_sql(&self, table_name: &str) -> Result<String, Exception> {
        let sql = self
            .connection
            .fetch_one(
                "SELECT sql\nFROM sqlite_master\nWHERE type = 'table'\n  AND name = ?",
                &PhpArray::list([Value::string(table_name)]),
                &ParameterTypes::new(),
            )
            .await?;

        match sql {
            None => Err(assertion_error("$sql !== false")),
            Some(sql) => text(Some(&sql)),
        }
    }

    async fn get_table_options(&self, table_names: Vec<String>) -> Rows<TableMetadataRow> {
        let mut result = Vec::new();
        for table_name in table_names {
            let create_sql = self.get_create_table_sql(&table_name).await?;

            let mut options = PhpArray::new();
            options.insert(
                "comment",
                self.parse_table_comment_from_sql(&table_name, &create_sql)?
                    .map_or(Value::Null, Value::string),
            );
            options.insert(
                "without_rowid",
                Value::Bool(parse_without_rowid_from_sql(&create_sql)?),
            );

            result.push(TableMetadataRow::new(None, &table_name, options));
        }
        Ok(result)
    }

    fn build_table_query_predicate(table_name: Option<&str>, params: &mut Vec<String>) -> String {
        let mut conditions = vec![
            "t.type = 'table'".to_owned(),
            Self::build_table_name_predicate("t.name"),
        ];

        if let Some(table_name) = table_name {
            conditions.push("t.name = ?".to_owned());
            params.push(table_name.to_owned());
        }

        conditions.join(" AND ")
    }

    fn parse_table_comment_from_sql(
        &self,
        table: &str,
        sql: &str,
    ) -> Result<Option<String>, Exception> {
        let pattern = format!(
            "/CREATE\\s+TABLE{}\n( # Start capture\n   (?:\\s*--[^\\n]*\\n?)+ # Capture anything that starts with whitespaces followed by -- until the end\n                       # of the line(s)\n)/ix",
            self.build_identifier_pattern(table)
        );

        let Some(matches) = preg_match(&pattern, sql)? else {
            return Ok(None);
        };

        let comment = preg_replace(
            r"{^\s*--}m",
            "",
            rtrim_newlines(group(&matches, 1).unwrap_or("")),
        )?;

        Ok(if comment.is_empty() {
            None
        } else {
            Some(comment)
        })
    }

    fn build_table_name_predicate(column_name: &str) -> String {
        format!("{column_name} NOT IN ('geometry_columns', 'spatial_ref_sys', 'sqlite_sequence')")
    }
}

/// The pattern of `getForeignKeyConstraintDetails()` (`/x`: the layout
/// is PHP's).
pub(crate) const FOREIGN_KEY_DETAILS_PATTERN: &str = r"#
                    (?:CONSTRAINT\s+(\S+)\s+)?
                    (?:FOREIGN\s+KEY[^)]+\)\s*)?
                    REFERENCES\s+\S+\s*(?:\([^)]+\))?
                    (?:
                        [^,]*?
                        (NOT\s+DEFERRABLE|DEFERRABLE)
                        (?:\s+INITIALLY\s+(DEFERRED|IMMEDIATE))?
                    )?#isx";

/// `(int) $string` of a run of digits.
pub(crate) fn php_int(digits: &str) -> i64 {
    digits.parse().unwrap_or(i64::MAX)
}

/// `buildIdentifierPattern($identifier)`.
pub(crate) fn build_identifier_pattern(platform: &SQLitePlatform, identifier: &str) -> String {
    let alternatives: Vec<String> = [
        identifier.to_owned(),
        platform.quote_single_identifier(identifier),
    ]
    .iter()
    .map(|sql| format!("\\W{}\\W", preg_quote(sql, '/')))
    .collect();
    format!("(?:{})", alternatives.join("|"))
}

/// `parseWithoutRowidFromSQL($sql)`.
pub(crate) fn parse_without_rowid_from_sql(sql: &str) -> Result<bool, Exception> {
    let Some(matches) = preg_match(r"/\)[^)]*$/s", sql)? else {
        return Ok(false);
    };

    Ok(preg_match(r"/\bWITHOUT\s+ROWID\b/i", group(&matches, 0).unwrap_or(""))?.is_some())
}

fn parse_default_expression(value: &str) -> Result<Option<String>, Exception> {
    if value == "NULL" {
        return Ok(None);
    }

    if let Some(matches) = preg_match(r"/^'(.*)'$/s", value)? {
        return Ok(Some(group(&matches, 1).unwrap_or("").replace("''", "'")));
    }

    Ok(Some(value.to_owned()))
}

fn create_referential_action(value: &str) -> Result<ReferentialAction, Exception> {
    ReferentialAction::try_from_value(value).ok_or_else(|| assertion_error("$action !== null"))
}

/// `parseOptionallyQuotedName($sql)`.
fn parse_optionally_quoted_name(sql: &str) -> Option<String> {
    if sql.is_empty() {
        return None;
    }

    if sql.len() >= 2 && sql.starts_with('"') && sql.ends_with('"') {
        return Some(sql[1..sql.len() - 1].replace("\"\"", "\""));
    }

    Some(sql.to_owned())
}

fn method(name: &str) -> String {
    format!("{CLASS}::{name}")
}

impl MetadataProvider for SQLiteMetadataProvider<'_> {
    async fn get_all_database_names(&self) -> Rows<DatabaseMetadataRow> {
        Err(NotSupported::new(&method("getAllDatabaseNames")).into())
    }

    async fn get_all_schema_names(&self) -> Rows<SchemaMetadataRow> {
        Err(NotSupported::new(&method("getAllSchemaNames")).into())
    }

    async fn get_all_table_names(&self) -> Rows<TableMetadataRow> {
        Ok(self
            .get_table_names()
            .await?
            .iter()
            .map(|table_name| TableMetadataRow::new(None, table_name, PhpArray::new()))
            .collect())
    }

    async fn get_table_columns_for_all_tables(&self) -> Rows<TableColumnMetadataRow> {
        self.get_table_columns(None).await
    }

    async fn get_table_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableColumnMetadataRow> {
        if let Some(schema_name) = schema_name {
            return Err(unsupported_name_from_non_null_schema_name(
                schema_name,
                &method("getTableColumnsForTable"),
            ));
        }
        self.get_table_columns(Some(table_name)).await
    }

    async fn get_index_columns_for_all_tables(&self) -> Rows<IndexColumnMetadataRow> {
        self.get_index_columns(None).await
    }

    async fn get_index_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<IndexColumnMetadataRow> {
        if let Some(schema_name) = schema_name {
            return Err(unsupported_name_from_non_null_schema_name(
                schema_name,
                &method("getIndexColumnsForTable"),
            ));
        }
        self.get_index_columns(Some(table_name)).await
    }

    async fn get_primary_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        self.get_primary_key_constraint_columns(None).await
    }

    async fn get_primary_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        if let Some(schema_name) = schema_name {
            return Err(unsupported_name_from_non_null_schema_name(
                schema_name,
                &method("getPrimaryKeyConstraintColumnsForTable"),
            ));
        }
        self.get_primary_key_constraint_columns(Some(table_name))
            .await
    }

    async fn get_unique_constraint_columns_for_all_tables(
        &self,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        Some(self.get_unique_constraint_columns(None).await)
    }

    async fn get_unique_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        if let Some(schema_name) = schema_name {
            return Some(Err(unsupported_name_from_non_null_schema_name(
                schema_name,
                &method("getUniqueConstraintColumnsForTable"),
            )));
        }
        Some(self.get_unique_constraint_columns(Some(table_name)).await)
    }

    async fn get_foreign_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        self.get_foreign_key_constraint_columns(None).await
    }

    async fn get_foreign_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        if let Some(schema_name) = schema_name {
            return Err(unsupported_name_from_non_null_schema_name(
                schema_name,
                &method("getForeignKeyConstraintColumnsForTable"),
            ));
        }
        self.get_foreign_key_constraint_columns(Some(table_name))
            .await
    }

    async fn get_table_options_for_all_tables(&self) -> Rows<TableMetadataRow> {
        let table_names = self.get_table_names().await?;
        self.get_table_options(table_names).await
    }

    async fn get_table_options_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableMetadataRow> {
        if let Some(schema_name) = schema_name {
            return Err(unsupported_name_from_non_null_schema_name(
                schema_name,
                &method("getTableOptionsForTable"),
            ));
        }
        self.get_table_options(vec![table_name.to_owned()]).await
    }

    async fn get_all_views(&self) -> Rows<ViewMetadataRow> {
        let sql =
            "SELECT name,\n       sql\nFROM sqlite_master\nWHERE type = 'view'\nORDER BY name";

        self.fetch_all_numeric(sql, &[])
            .await?
            .iter()
            .map(|row| {
                Ok(ViewMetadataRow::new(
                    None,
                    &text(cell(row, 0))?,
                    &text(cell(row, 1))?,
                ))
            })
            .collect()
    }

    async fn get_all_sequences(&self) -> Rows<SequenceMetadataRow> {
        Err(NotSupported::new(&method("getAllSequences")).into())
    }

    fn class(&self) -> &str {
        CLASS
    }
}
