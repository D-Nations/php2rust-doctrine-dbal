//! Port of Doctrine\DBAL\Platforms\OraclePlatform.php: the Oracle
//! dialect.
//!
//! A column definition's `sequence` (a `Sequence` object in PHP) is not
//! read: a column array that sets it gives an `UNSUPPORTED` error. The
//! deprecations `createReservedKeywordsList()` and
//! `getCreatePrimaryKeySQL()` raise when called from outside the package
//! are not raised: the trait method is the one the package calls.
//! `createMetadataProvider()` and `createSchemaManager()` are not ported
//! (no Oracle connection).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/OraclePlatform.php

use std::fmt::Write as _;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::TransactionIsolationLevel;
use crate::deprecations;
use crate::exception::Exception;
use crate::schema::name::UnquotedIdentifierFolding;
use crate::schema::{ForeignKeyConstraint, Identifier, Index, Sequence, TableDiff};
use crate::types::{self, Types};

use super::abstract_platform::{AbstractPlatform, parent, validate_create_table_options};
use super::date_interval_unit::DateIntervalUnit;
use super::keywords::{KeywordList, OracleKeywords};
use super::platform_state::{
    CreateTableOptions, PlatformState, concat_value, string_argument, utf8,
};
use super::sqlite_platform::GLOBAL_INVALID_ARGUMENT_EXCEPTION;

/// `OraclePlatform`.
#[derive(Debug, Default)]
pub struct OraclePlatform {
    state: PlatformState,
}

impl OraclePlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\OraclePlatform";

    pub fn new() -> Self {
        Self::default()
    }

    /// `getSequenceCacheSQL()` (private).
    fn get_sequence_cache_sql(cache_size: Option<i64>) -> String {
        match cache_size {
            Some(0 | 1) => " NOCACHE".to_owned(),
            Some(cache_size) if cache_size > 1 => format!(" CACHE {cache_size}"),
            _ => String::new(),
        }
    }

    /// `getCreateAutoincrementSql()` (protected).
    fn get_create_autoincrement_sql(
        &self,
        name: &str,
        table: &str,
        start: i64,
    ) -> Result<Vec<String>, Exception> {
        let table_identifier = Self::normalize_identifier(table);
        let quoted_table_name = table_identifier.get_quoted_name(self);
        let unquoted_table_name = table_identifier.get_name();

        let name_identifier = Self::normalize_identifier(name);
        let quoted_name = name_identifier.get_quoted_name(self);

        let mut sql = Vec::new();

        let autoincrement_identifier_name =
            self.get_autoincrement_identifier_name(&table_identifier);

        let idx = Index::new(
            Some(&autoincrement_identifier_name),
            [quoted_name.clone()],
            true,
            true,
            Vec::<String>::new(),
            PhpArray::new(),
        )?;

        sql.push(format!(
            "DECLARE
  CONSTRAINTS_COUNT NUMBER;
BEGIN
  SELECT COUNT(CONSTRAINT_NAME) INTO CONSTRAINTS_COUNT
    FROM USER_CONSTRAINTS
   WHERE TABLE_NAME = {}
     AND CONSTRAINT_TYPE = 'P';
  IF CONSTRAINTS_COUNT = 0 THEN
    EXECUTE IMMEDIATE {};
  END IF;
END;",
            self.quote_string_literal(&unquoted_table_name),
            self.quote_string_literal(&self.get_create_index_sql(&idx, &quoted_table_name)?)
        ));

        let sequence_name = self.get_identity_sequence_name(if table_identifier.is_quoted() {
            &quoted_table_name
        } else {
            &unquoted_table_name
        });
        let sequence = Sequence::with_options(&sequence_name, start, 1, None);
        sql.push(self.get_create_sequence_sql(&sequence)?);

        sql.push(format!(
            "CREATE TRIGGER {autoincrement_identifier_name}
   BEFORE INSERT
   ON {quoted_table_name}
   FOR EACH ROW
DECLARE
   last_Sequence NUMBER;
   last_InsertID NUMBER;
BEGIN
   IF (:NEW.{quoted_name} IS NULL OR :NEW.{quoted_name} = 0) THEN
      SELECT {sequence_name}.NEXTVAL INTO :NEW.{quoted_name} FROM DUAL;
   ELSE
      SELECT NVL(Last_Number, 0) INTO last_Sequence
        FROM USER_SEQUENCES
       WHERE Sequence_Name = {};
      SELECT :NEW.{quoted_name} INTO last_InsertID FROM DUAL;
      WHILE (last_InsertID > last_Sequence) LOOP
         SELECT {sequence_name}.NEXTVAL INTO last_Sequence FROM DUAL;
      END LOOP;
   END IF;
END;",
            self.quote_string_literal(&sequence.get_name())
        ));

        Ok(sql)
    }

    /// Returns the SQL statements to drop the autoincrement for the given
    /// table name (`@internal`, for the schema manager).
    pub fn get_drop_autoincrement_sql(&self, table: &str) -> Result<Vec<String>, Exception> {
        let table = Self::normalize_identifier(table);
        let autoincrement_identifier_name = self.get_autoincrement_identifier_name(&table);
        let identity_sequence_name = self.get_identity_sequence_name(&if table.is_quoted() {
            table.get_quoted_name(self)
        } else {
            table.get_name()
        });

        Ok(vec![
            format!("DROP TRIGGER {autoincrement_identifier_name}"),
            self.get_drop_sequence_sql(&identity_sequence_name)?,
            self.get_drop_constraint_sql(
                &autoincrement_identifier_name,
                &table.get_quoted_name(self),
            ),
        ])
    }

    /// `normalizeIdentifier()` (private): the name as an identifier,
    /// upper-cased unless it is quoted.
    fn normalize_identifier(name: &str) -> Identifier {
        let identifier = Identifier::new(name, false);

        if identifier.is_quoted() {
            identifier
        } else {
            Identifier::new(&name.to_ascii_uppercase(), false)
        }
    }

    /// `addSuffix()` (private): adds the suffix, cutting the identifier
    /// to the maximum identifier length first.
    fn add_suffix(&self, identifier: &str, suffix: &str) -> String {
        let max_possible_length_without_suffix = usize::try_from(self.get_max_identifier_length())
            .unwrap_or(0)
            .saturating_sub(suffix.len());
        let mut bytes = identifier.as_bytes();
        if bytes.len() > max_possible_length_without_suffix {
            bytes = &bytes[..max_possible_length_without_suffix];
        }
        let mut out = String::from_utf8_lossy(bytes).into_owned();
        out.push_str(suffix);
        out
    }

    /// `getAutoincrementIdentifierName()` (private).
    fn get_autoincrement_identifier_name(&self, table: &Identifier) -> String {
        let identifier_name = self.add_suffix(&table.get_name(), "_AI_PK");

        if table.is_quoted() {
            self.quote_single_identifier(&identifier_name)
        } else {
            identifier_name
        }
    }

    /// `getIdentitySequenceName()` (protected).
    fn get_identity_sequence_name(&self, table_name: &str) -> String {
        let table = Identifier::new(table_name, false);

        // No usage of column name to preserve BC compatibility with <2.5
        let mut identity_sequence_name = self.add_suffix(&table.get_name(), "_SEQ");

        if table.is_quoted() {
            identity_sequence_name = format!("\"{identity_sequence_name}\"");
        }

        let identity_sequence_identifier = Self::normalize_identifier(&identity_sequence_name);

        identity_sequence_identifier.get_quoted_name(self)
    }
}

impl AbstractPlatform for OraclePlatform {
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

    fn get_substring_expression(&self, string: &str, start: &str, length: Option<&str>) -> String {
        match length {
            None => format!("SUBSTR({string}, {start})"),
            Some(length) => format!("SUBSTR({string}, {start}, {length})"),
        }
    }

    fn get_locate_expression(
        &self,
        string: &str,
        substring: &str,
        start: Option<&str>,
    ) -> Result<String, Exception> {
        Ok(match start {
            None => format!("INSTR({string}, {substring})"),
            Some(start) => format!("INSTR({string}, {substring}, {start})"),
        })
    }

    fn get_date_arithmetic_interval_expression(
        &self,
        date: &str,
        operator: &str,
        interval: &str,
        unit: DateIntervalUnit,
    ) -> Result<String, Exception> {
        Ok(match unit {
            DateIntervalUnit::Month | DateIntervalUnit::Quarter | DateIntervalUnit::Year => {
                let interval = match unit {
                    DateIntervalUnit::Quarter => self.multiply_interval(interval, 3),
                    DateIntervalUnit::Year => self.multiply_interval(interval, 12),
                    _ => interval.to_owned(),
                };

                format!("ADD_MONTHS({date}, {operator}{interval})")
            }
            _ => {
                let calculation_clause = match unit {
                    DateIntervalUnit::Second => "/24/60/60",
                    DateIntervalUnit::Minute => "/24/60",
                    DateIntervalUnit::Hour => "/24",
                    DateIntervalUnit::Week => "*7",
                    _ => "",
                };

                format!("({date}{operator}{interval}{calculation_clause})")
            }
        })
    }

    fn get_date_diff_expression(&self, date1: &str, date2: &str) -> Result<String, Exception> {
        Ok(format!("TRUNC({date1}) - TRUNC({date2})"))
    }

    fn get_bit_and_comparison_expression(&self, value1: &str, value2: &str) -> String {
        format!("BITAND({value1}, {value2})")
    }

    fn get_current_database_expression(&self) -> Result<String, Exception> {
        Ok("SYS_CONTEXT('USERENV', 'CURRENT_SCHEMA')".to_owned())
    }

    fn get_bit_or_comparison_expression(&self, value1: &str, value2: &str) -> String {
        format!(
            "({value1}-{}+{value2})",
            self.get_bit_and_comparison_expression(value1, value2)
        )
    }

    /// Deprecated upstream.
    fn get_create_primary_key_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        Ok(format!(
            "ALTER TABLE {table} ADD CONSTRAINT {} PRIMARY KEY ({})",
            index.get_quoted_name(self),
            index.get_quoted_columns(self).join(", ")
        ))
    }

    fn get_create_sequence_sql(&self, sequence: &Sequence) -> Result<String, Exception> {
        Ok(format!(
            "CREATE SEQUENCE {} START WITH {} MINVALUE {} INCREMENT BY {}{}",
            sequence.get_quoted_name(self),
            sequence.get_initial_value(),
            sequence.get_initial_value(),
            sequence.get_allocation_size(),
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

    fn get_sequence_next_val_sql(&self, sequence: &str) -> Result<String, Exception> {
        Ok(format!("SELECT {sequence}.nextval FROM DUAL"))
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

    fn get_transaction_isolation_level_sql(&self, level: TransactionIsolationLevel) -> String {
        match level {
            TransactionIsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            TransactionIsolationLevel::ReadCommitted => "READ COMMITTED",
            TransactionIsolationLevel::RepeatableRead | TransactionIsolationLevel::Serializable => {
                "SERIALIZABLE"
            }
        }
        .to_owned()
    }

    fn get_boolean_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("NUMBER(1)"))
    }

    fn get_integer_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("NUMBER(10)"))
    }

    fn get_big_int_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("NUMBER(20)"))
    }

    fn get_small_int_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("NUMBER(5)"))
    }

    fn get_date_time_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIMESTAMP(0)"))
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
        Ok(PhpString::from("DATE"))
    }

    fn get_common_integer_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from(""))
    }

    fn get_varchar_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        match length {
            None => Err(Exception::column_length_required(self.class(), "VARCHAR2")),
            Some(length) => Ok(PhpString::from(format!("VARCHAR2({length})"))),
        }
    }

    fn get_binary_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        match length {
            None => Err(Exception::column_length_required(self.class(), "RAW")),
            Some(length) => Ok(PhpString::from(format!("RAW({length})"))),
        }
    }

    fn get_varbinary_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        self.get_binary_type_declaration_sql_snippet(length)
    }

    fn get_clob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("CLOB"))
    }

    fn get_list_databases_sql(&self) -> Result<String, Exception> {
        Ok("SELECT username FROM all_users".to_owned())
    }

    fn get_list_sequences_sql(&self, database: &str) -> Result<String, Exception> {
        Ok(format!(
            "SELECT SEQUENCE_NAME, MIN_VALUE, INCREMENT_BY FROM SYS.ALL_SEQUENCES WHERE SEQUENCE_OWNER = {}",
            self.quote_string_literal(&Self::normalize_identifier(database).get_name())
        ))
    }

    fn get_create_table_sql_internal(
        &self,
        name: &str,
        columns: &[PhpArray],
        options: CreateTableOptions,
    ) -> Result<Vec<String>, Exception> {
        validate_create_table_options(
            &options,
            "Doctrine\\DBAL\\Platforms\\OraclePlatform::_getCreateTableSQL",
        );

        let mut options = options;
        let indexes = options.indexes.replace(Vec::new()).unwrap_or_default();
        let mut sql = parent::get_create_table_sql_internal(self, name, columns, options)?;

        for column in columns {
            if column
                .get(&Key::from("sequence"))
                .is_some_and(|sequence| !sequence.is_null())
            {
                return Err(Exception::new(
                    Exception::UNSUPPORTED,
                    "a column definition with a \"sequence\"",
                ));
            }

            if column
                .get(&Key::from("autoincrement"))
                .is_some_and(|autoincrement| {
                    !autoincrement.is_null() && !matches!(autoincrement, Value::Bool(false))
                })
            {
                let column_name = string_argument(
                    column.get(&Key::from("name")).unwrap_or(&Value::Null),
                    "Doctrine\\DBAL\\Platforms\\OraclePlatform::getCreateAutoincrementSql",
                    1,
                    "name",
                )?;
                sql.extend(self.get_create_autoincrement_sql(&column_name, name, 1)?);
            }
        }

        for index in &indexes {
            sql.push(self.get_create_index_sql(index, name)?);
        }

        Ok(sql)
    }

    fn get_list_views_sql(&self, _database: &str) -> Result<String, Exception> {
        Ok("SELECT view_name, text FROM sys.user_views".to_owned())
    }

    fn get_drop_foreign_key_sql(
        &self,
        foreign_key: &str,
        table: &str,
    ) -> Result<String, Exception> {
        Ok(self.get_drop_constraint_sql(foreign_key, table))
    }

    fn get_advanced_foreign_key_options_sql(
        &self,
        foreign_key: &ForeignKeyConstraint,
    ) -> Result<String, Exception> {
        let mut sql = String::new();

        if foreign_key.has_option_inner("onDelete") {
            let action = string_argument(
                &foreign_key.option("onDelete"),
                "Doctrine\\DBAL\\Platforms\\OraclePlatform::getForeignKeyReferentialActionSQL",
                1,
                "action",
            )?;
            let referential_action = self.get_foreign_key_referential_action_sql(&action)?;

            if !referential_action.is_empty() {
                sql.push_str(" ON DELETE ");
                sql.push_str(&referential_action);
            }
        }

        let deferrability_sql = self.get_constraint_deferrability_sql(foreign_key);

        if !deferrability_sql.is_empty() {
            sql.push_str(&deferrability_sql);
        }

        Ok(sql)
    }

    fn get_foreign_key_referential_action_sql(&self, action: &str) -> Result<String, Exception> {
        let action = action.to_ascii_uppercase();

        if action == "RESTRICT" {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6707", || {
                "Relying on automatic conversion of RESTRICT to NO ACTION for Oracle is deprecated. Use NO ACTION explicitly instead.".to_owned()
            });
        }

        match action.as_str() {
            "RESTRICT" | "NO ACTION" => Ok(String::new()),
            "CASCADE" | "SET NULL" => Ok(action),
            _ => Err(Exception::new(
                GLOBAL_INVALID_ARGUMENT_EXCEPTION,
                format!("Invalid foreign key action \"{action}\"."),
            )),
        }
    }

    fn get_create_database_sql(&self, name: &str) -> Result<String, Exception> {
        Ok(format!("CREATE USER {name}"))
    }

    fn get_drop_database_sql(&self, name: &str) -> Result<String, Exception> {
        Ok(format!("DROP USER {name} CASCADE"))
    }

    fn get_alter_table_sql(&self, diff: &TableDiff) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        let mut comments_sql = Vec::new();
        let mut add_column_sql = Vec::new();

        let table_name_sql = diff.get_old_table().get_quoted_name(self);

        for column in diff.get_added_columns() {
            add_column_sql.push(self.get_column_declaration_sql(
                &column.get_quoted_name(self),
                &column.to_array(true)?,
            )?);
            let comment = column.get_comment();

            if !comment.is_empty() {
                comments_sql.push(self.get_comment_on_column_sql(
                    &table_name_sql,
                    &column.get_quoted_name(self),
                    comment,
                ));
            }
        }

        if !add_column_sql.is_empty() {
            sql.push(format!(
                "ALTER TABLE {table_name_sql} ADD ({})",
                add_column_sql.join(", ")
            ));
        }

        let mut modify_column_sql = Vec::new();
        for column_diff in diff.get_changed_columns() {
            let new_column = column_diff.get_new_column();
            let old_column = column_diff.get_old_column();

            // Column names in Oracle are case insensitive and automatically uppercased on the server.
            if column_diff.has_name_changed() {
                let new_column_name = new_column.get_quoted_name(self);
                let old_column_name = old_column.get_quoted_name(self);

                sql.extend(self.get_rename_column_sql(
                    &table_name_sql,
                    &old_column_name,
                    &new_column_name,
                ));
            }

            let count_changed_properties = column_diff.count_changed_properties();
            // Do not generate column alteration clause if type is binary and only fixed property has changed.
            // Oracle only supports binary type columns with variable length.
            // Avoids unnecessary table alteration statements.
            let new_type = types::get_type(new_column.get_type_name()).map_err(Exception::from)?;
            if new_type.class() == "Doctrine\\DBAL\\Types\\BinaryType"
                && column_diff.has_fixed_changed()
                && count_changed_properties == 1
            {
                continue;
            }

            let column_has_changed_comment = column_diff.has_comment_changed();

            if count_changed_properties > usize::from(column_has_changed_comment) {
                let mut new_column_properties = new_column.to_array(true)?;

                let old_sql = self.get_column_declaration_sql("", &old_column.to_array(true)?)?;
                let mut new_sql = self.get_column_declaration_sql("", &new_column_properties)?;

                if new_sql != old_sql {
                    if !column_diff.has_not_null_changed() {
                        new_column_properties.remove(&Key::from("notnull"));
                        new_sql = self.get_column_declaration_sql("", &new_column_properties)?;
                    }

                    modify_column_sql
                        .push(format!("{}{new_sql}", new_column.get_quoted_name(self)));
                }
            }

            if column_diff.has_comment_changed() {
                comments_sql.push(self.get_comment_on_column_sql(
                    &table_name_sql,
                    &new_column.get_quoted_name(self),
                    new_column.get_comment(),
                ));
            }
        }

        if !modify_column_sql.is_empty() {
            sql.push(format!(
                "ALTER TABLE {table_name_sql} MODIFY ({})",
                modify_column_sql.join(", ")
            ));
        }

        let drop_column_sql: Vec<String> = diff
            .get_dropped_columns()
            .iter()
            .map(|column| column.get_quoted_name(self))
            .collect();

        if !drop_column_sql.is_empty() {
            sql.push(format!(
                "ALTER TABLE {table_name_sql} DROP ({})",
                drop_column_sql.join(", ")
            ));
        }

        let mut result = self.get_pre_alter_table_index_foreign_key_sql(diff)?;
        result.extend(sql);
        result.extend(comments_sql);
        result.extend(self.get_post_alter_table_index_foreign_key_sql(diff)?);
        Ok(result)
    }

    fn get_column_declaration_sql(
        &self,
        name: &str,
        column: &PhpArray,
    ) -> Result<String, Exception> {
        let declaration = match column.get(&Key::from("columnDefinition")) {
            Some(definition) if !definition.is_null() => concat_value(definition)?,
            _ => {
                let default = self.get_default_value_declaration_sql(column)?;

                let notnull = match column.get(&Key::from("notnull")) {
                    Some(notnull) if !notnull.is_null() => {
                        if notnull.is_truthy() {
                            " NOT NULL"
                        } else {
                            " NULL"
                        }
                    }
                    _ => "",
                };

                let type_decl = utf8(
                    &self
                        .get_column_type(column)?
                        .get_sql_declaration(column, self)?,
                )?;
                format!("{type_decl}{default}{notnull}")
            }
        };

        Ok(format!("{name} {declaration}"))
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

    fn supports_comment_on_statement(&self) -> bool {
        true
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

    fn get_create_temporary_table_snippet_sql(&self) -> String {
        "CREATE GLOBAL TEMPORARY TABLE".to_owned()
    }

    fn get_date_time_tz_format_string(&self) -> String {
        "Y-m-d H:i:sP".to_owned()
    }

    fn get_date_format_string(&self) -> String {
        "Y-m-d 00:00:00".to_owned()
    }

    fn get_time_format_string(&self) -> String {
        "1900-01-01 H:i:s".to_owned()
    }

    fn get_max_identifier_length(&self) -> i64 {
        128
    }

    fn supports_sequences(&self) -> bool {
        true
    }

    fn supports_release_savepoints(&self) -> bool {
        false
    }

    fn get_truncate_table_sql(&self, table_name: &str, _cascade: bool) -> String {
        let table_identifier = Identifier::new(table_name, false);

        format!("TRUNCATE TABLE {}", table_identifier.get_quoted_name(self))
    }

    fn get_dummy_select_sql(&self, expression: &str) -> String {
        format!("SELECT {expression} FROM DUAL")
    }

    fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("binary_double", Types::FLOAT),
            ("binary_float", Types::FLOAT),
            ("binary_integer", Types::BOOLEAN),
            ("blob", Types::BLOB),
            ("char", Types::STRING),
            ("clob", Types::TEXT),
            ("date", Types::DATE_MUTABLE),
            ("float", Types::FLOAT),
            ("integer", Types::INTEGER),
            ("long", Types::STRING),
            ("long raw", Types::BLOB),
            ("nchar", Types::STRING),
            ("nclob", Types::TEXT),
            ("number", Types::INTEGER),
            ("nvarchar2", Types::STRING),
            ("pls_integer", Types::BOOLEAN),
            ("raw", Types::BINARY),
            ("real", Types::SMALLFLOAT),
            ("rowid", Types::STRING),
            ("timestamp", Types::DATETIME_MUTABLE),
            ("timestamptz", Types::DATETIMETZ_MUTABLE),
            ("urowid", Types::STRING),
            ("varchar", Types::STRING),
            ("varchar2", Types::STRING),
        ]
    }

    fn release_save_point(&self, _savepoint: &str) -> String {
        String::new()
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        OracleKeywords::new()
    }

    fn get_blob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BLOB"))
    }
}
