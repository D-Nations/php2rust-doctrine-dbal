//! Port of Doctrine\DBAL\Platforms\MySQL\MySQLMetadataProvider.php: the
//! `MySQL` and `MariaDB` metadata of databases, tables, columns, indexes,
//! constraints and views, read from `information_schema`.
//!
//! The constructor reads the current database name, so it is `async` and
//! fallible. PHP's generators are the rows collected into a `Vec`. The
//! platform is a `&dyn AbstractMySQLPlatform`; `instanceof
//! MariaDBPlatform` is `is_mariadb()`. PHP's `assert()` calls (enabled in
//! DBAL's test suite) fail with an `AssertionError`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/MySQLMetadataProvider.php

use php_value::{Key, PhpArray, Value};

use crate::exception::Exception;
use crate::platforms::sqlite::preg::{group, preg_match, preg_match_all};
use crate::platforms::sqlite::{assertion_error, nullable_text, text_value};
use crate::platforms::{
    AbstractMySQLPlatform, LENGTH_LIMIT_BLOB, LENGTH_LIMIT_MEDIUMBLOB, LENGTH_LIMIT_MEDIUMTEXT,
    LENGTH_LIMIT_TEXT, LENGTH_LIMIT_TINYBLOB, LENGTH_LIMIT_TINYTEXT, NotSupported,
};
use crate::schema::exception::unsupported_name_from_non_null_schema_name;
use crate::schema::metadata::{
    DatabaseMetadataRow, ForeignKeyConstraintColumnMetadataRow,
    ForeignKeyConstraintColumnMetadataRowParts, IndexColumnMetadataRow, MetadataProvider,
    PrimaryKeyConstraintColumnRow, SchemaMetadataRow, SequenceMetadataRow, TableColumnMetadataRow,
    TableMetadataRow, UniqueConstraintColumnMetadataRow, ViewMetadataRow,
};
use crate::schema::{Column, IndexType, MatchType, ReferentialAction};
use crate::{Connection, ParameterTypes};

const CLASS: &str = "Doctrine\\DBAL\\Platforms\\MySQL\\MySQLMetadataProvider";

type Rows<T> = Result<Vec<T>, Exception>;

/// @see <https://mariadb.com/kb/en/library/string-literals/#escape-sequences>
pub(crate) const MARIADB_ESCAPE_SEQUENCES: &[(&str, &str)] = &[
    ("\\0", "\0"),
    ("\\'", "'"),
    ("\\\"", "\""),
    ("\\b", "\u{8}"),
    ("\\n", "\n"),
    ("\\r", "\r"),
    ("\\t", "\t"),
    ("\\Z", "\u{1a}"),
    ("\\\\", "\\"),
    ("\\%", "%"),
    ("\\_", "_"),
    // Internally, MariaDB escapes single quotes using the standard syntax
    ("''", "'"),
];

fn method(name: &str) -> String {
    format!("{CLASS}::{name}")
}

/// `strtr($subject, $pairs)`: at each position the longest matching key
/// is replaced, and the replacement is not searched again.
pub(crate) fn strtr(subject: &str, pairs: &[(&str, &str)]) -> String {
    let mut pairs: Vec<&(&str, &str)> = pairs.iter().filter(|(from, _)| !from.is_empty()).collect();
    pairs.sort_by_key(|(from, _)| std::cmp::Reverse(from.len()));

    let bytes = subject.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    'outer: while i < bytes.len() {
        for (from, to) in &pairs {
            if bytes[i..].starts_with(from.as_bytes()) {
                out.extend_from_slice(to.as_bytes());
                i += from.len();
                continue 'outer;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `parseEnumExpression($expression)`.
pub(crate) fn parse_enum_expression(expression: &str) -> Result<Vec<String>, Exception> {
    let matches = preg_match_all("/'([^']*(?:''[^']*)*)'/", expression)?;

    Ok(matches
        .get(1)
        .map(|values| {
            values
                .iter()
                .map(|value| strtr(value, &[("''", "'")]))
                .collect()
        })
        .unwrap_or_default())
}

/// `parseCreateOptions($string)`.
pub(crate) fn parse_create_options(string: Option<&str>) -> PhpArray {
    let mut options = PhpArray::new();

    let Some(string) = string.filter(|s| !s.is_empty()) else {
        return options;
    };

    for pair in string.split(' ') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().map_or(Value::Bool(true), Value::string);
        options.insert(Key::from(key), value);
    }

    options
}

/// `$row[$index]`.
fn cell(row: &PhpArray, index: i64) -> Option<&Value> {
    row.get(&index.into())
}

fn cell_text(row: &PhpArray, index: i64) -> Result<String, Exception> {
    text_value(cell(row, index))
}

fn cell_value(row: &PhpArray, index: i64) -> Value {
    cell(row, index).cloned().unwrap_or(Value::Null)
}

/// `$value === $s`.
fn is_string(value: Option<&Value>, s: &str) -> bool {
    matches!(value, Some(Value::String(v)) if v.as_bytes() == s.as_bytes())
}

/// `(int) $value`.
fn int(value: Option<&Value>) -> i64 {
    value.map_or(0, php_value::ops::to_int)
}

/// The `MySQL` metadata provider.
#[derive(Clone)]
pub struct MySQLMetadataProvider<'c> {
    connection: &'c Connection,
    platform: &'c dyn AbstractMySQLPlatform,
    database_name: String,
}

impl std::fmt::Debug for MySQLMetadataProvider<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MySQLMetadataProvider")
            .field("platform", &self.platform.class())
            .field("database_name", &self.database_name)
            .finish_non_exhaustive()
    }
}

impl<'c> MySQLMetadataProvider<'c> {
    /// `@internal` This class can be instantiated only by a database
    /// platform. Reads the current database name (`SELECT DATABASE()`);
    /// no database selected is a `DatabaseRequired` exception.
    pub async fn new(
        connection: &'c Connection,
        platform: &'c dyn AbstractMySQLPlatform,
    ) -> Result<Self, Exception> {
        let database_name = connection
            .fetch_one(
                "SELECT DATABASE()",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?;

        match database_name {
            None | Some(Value::Null) => Err(Exception::database_required(&method("__construct"))),
            Some(database_name) => Ok(Self {
                connection,
                platform,
                database_name: text_value(Some(&database_name))?,
            }),
        }
    }

    async fn fetch_all_numeric(&self, sql: &str, params: &[&str]) -> Rows<PhpArray> {
        let params = PhpArray::list(params.iter().map(|p| Value::string(*p)));
        self.connection
            .fetch_all_numeric(sql, &params, &ParameterTypes::new())
            .await
    }

    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-columns-table.html>
    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-tables-table.html>
    async fn get_table_columns(&self, table_name: Option<&str>) -> Rows<TableColumnMetadataRow> {
        // The schema name is passed multiple times in the WHERE clause instead of using a JOIN condition to avoid
        // performance issues on MySQL older than 8.0 and the corresponding MariaDB versions caused by
        // https://bugs.mysql.com/bug.php?id=81347
        let mut conditions = vec!["c.TABLE_SCHEMA = ?", "t.TABLE_SCHEMA = ?"];
        let mut params = vec![self.database_name.as_str(), self.database_name.as_str()];

        if let Some(table_name) = table_name {
            conditions.push("t.TABLE_NAME = ?");
            params.push(table_name);
        }

        // On MySQL < 8.0 and MariaDB, information_schema compares object names case-insensitively and so
        // conflates table names that differ only in case. The binary comparison prevents that.
        let sql = format!(
            "SELECT c.TABLE_NAME,
       c.COLUMN_NAME,
       {},
       c.COLUMN_TYPE,
       c.CHARACTER_MAXIMUM_LENGTH,
       c.CHARACTER_OCTET_LENGTH,
       c.NUMERIC_PRECISION,
       c.NUMERIC_SCALE,
       c.IS_NULLABLE,
       c.COLUMN_DEFAULT,
       c.EXTRA,
       c.COLUMN_COMMENT,
       c.CHARACTER_SET_NAME,
       c.COLLATION_NAME
FROM information_schema.COLUMNS c
         INNER JOIN information_schema.TABLES t
                    ON t.TABLE_NAME = c.TABLE_NAME
                        AND CONVERT(t.TABLE_NAME USING binary) = CONVERT(c.TABLE_NAME USING binary)
WHERE {}
  AND t.TABLE_TYPE = 'BASE TABLE'
ORDER BY c.TABLE_NAME,
         c.ORDINAL_POSITION",
            self.platform
                .get_column_type_sql_snippet("c", &self.database_name),
            conditions.join(" AND "),
        );

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| self.create_table_column(row))
            .collect()
    }

    /// `createTableColumn($row)`.
    fn create_table_column(&self, row: &PhpArray) -> Result<TableColumnMetadataRow, Exception> {
        let table_name = cell_text(row, 0)?;
        let column_name = cell_text(row, 1)?;
        let db_type = cell_text(row, 2)?;
        let column_type = cell_text(row, 3)?;
        let character_maximum_length = cell(row, 4);
        let character_octet_length = cell(row, 5);
        let numeric_precision = cell(row, 6);
        let numeric_scale = cell(row, 7);
        let is_nullable = cell_text(row, 8)?;
        let column_default = nullable_text(cell(row, 9))?;
        let extra = cell_text(row, 10)?;
        let column_comment = cell_text(row, 11)?;
        let character_set_name = nullable_text(cell(row, 12))?;
        let collation_name = nullable_text(cell(row, 13))?;

        let mut editor = Column::editor()
            .set_quoted_name(&column_name)
            .set_type_name(&self.platform.doctrine_type_mapping(&db_type)?);

        if column_type.contains("unsigned") {
            editor = editor.set_unsigned(true);
        }

        match db_type.as_str() {
            "char" | "varchar" => {
                editor = editor.set_length(Some(int(character_maximum_length)));
            }
            "binary" | "varbinary" => {
                editor = editor.set_length(Some(int(character_octet_length)));
            }
            "tinytext" => editor = editor.set_length(Some(LENGTH_LIMIT_TINYTEXT)),
            "text" => editor = editor.set_length(Some(LENGTH_LIMIT_TEXT)),
            "mediumtext" => editor = editor.set_length(Some(LENGTH_LIMIT_MEDIUMTEXT)),
            "tinyblob" => editor = editor.set_length(Some(LENGTH_LIMIT_TINYBLOB)),
            "blob" => editor = editor.set_length(Some(LENGTH_LIMIT_BLOB)),
            "mediumblob" => editor = editor.set_length(Some(LENGTH_LIMIT_MEDIUMBLOB)),
            "float" | "double" | "real" | "numeric" | "decimal" => {
                editor = editor.set_precision(Some(int(numeric_precision)));

                if numeric_scale.is_some_and(|v| !v.is_null()) {
                    editor = editor.set_scale(int(numeric_scale));
                }
            }
            _ => {}
        }

        match db_type.as_str() {
            "char" | "binary" => editor = editor.set_fixed(true),
            "enum" => {
                editor = editor.set_values(PhpArray::list(
                    parse_enum_expression(&column_type)?
                        .into_iter()
                        .map(Value::string),
                ));
            }
            _ => {}
        }

        let default = if self.platform.is_mariadb() {
            self.parse_mariadb_column_default(column_default.as_deref())?
        } else {
            column_default
        };

        editor = editor
            .set_default_value(default.map_or(Value::Null, Value::string))
            .set_not_null(is_nullable != "YES")
            .set_comment(&column_comment)
            .set_charset(character_set_name.as_deref())
            .set_collation(collation_name.as_deref());

        if extra.contains("auto_increment") {
            editor = editor.set_autoincrement(true);
        }

        Ok(TableColumnMetadataRow::new(
            None,
            &table_name,
            editor.create()?,
        ))
    }

    /// Return Doctrine/Mysql-compatible column default values for `MariaDB`
    /// 10.2.7+ servers.
    ///
    /// - Since `MariaDB` 10.2.7 column defaults stored in
    ///   `information_schema` are quoted to distinguish them from
    ///   expressions.
    /// - The `CURRENT_TIMESTAMP`, `CURRENT_TIME` and `CURRENT_DATE`
    ///   expressions are represented as "`current_timestamp()`",
    ///   "`curdate()`" and "`curtime()`" respectively.
    /// - Quoted 'NULL' is not enforced. It is technically possible to have
    ///   "null" in some circumstances.
    /// - Single quotes are always escaped by doubling, even if the original
    ///   DDL used backslash escaping.
    ///
    /// @link <https://mariadb.com/kb/en/library/information-schema-columns-table/>
    /// @link <https://jira.mariadb.org/browse/MDEV-10134>
    /// @link <https://jira.mariadb.org/browse/MDEV-13132>
    /// @link <https://jira.mariadb.org/browse/MDEV-14053>
    fn parse_mariadb_column_default(
        &self,
        column_default: Option<&str>,
    ) -> Result<Option<String>, Exception> {
        let Some(column_default) = column_default.filter(|d| *d != "NULL") else {
            return Ok(None);
        };

        if let Some(matches) = preg_match(r"/^'(.*)'$/", column_default)? {
            return Ok(Some(strtr(
                group(&matches, 1).unwrap_or(""),
                MARIADB_ESCAPE_SEQUENCES,
            )));
        }

        Ok(Some(match column_default {
            "current_timestamp()" => self.platform.get_current_timestamp_sql(),
            "curdate()" => self.platform.get_current_date_sql(),
            "curtime()" => self.platform.get_current_time_sql(),
            default => default.to_owned(),
        }))
    }

    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-statistics-table.html>
    async fn get_index_columns(&self, table_name: Option<&str>) -> Rows<IndexColumnMetadataRow> {
        let mut conditions = vec!["TABLE_SCHEMA = ?"];
        let mut params = vec![self.database_name.as_str()];

        if let Some(table_name) = table_name {
            conditions.push("TABLE_NAME = ?");
            params.push(table_name);
        }

        let sql = format!(
            "SELECT TABLE_NAME,
       INDEX_NAME,
       INDEX_TYPE,
       NON_UNIQUE,
       COLUMN_NAME,
       SUB_PART
FROM information_schema.STATISTICS
WHERE {}
  AND INDEX_NAME != 'PRIMARY'
ORDER BY TABLE_NAME,
    INDEX_NAME,
    SEQ_IN_INDEX",
            conditions.join(" AND "),
        );

        let mut rows = Vec::new();
        for row in self.fetch_all_numeric(&sql, &params).await? {
            let mut length = match cell(&row, 5) {
                None | Some(Value::Null) => None,
                sub_part => {
                    let length = int(sub_part);
                    if length <= 0 {
                        return Err(assertion_error("$length > 0"));
                    }
                    Some(length)
                }
            };

            let index_type = cell(&row, 2);
            let type_ = if is_string(index_type, "FULLTEXT") {
                IndexType::Fulltext
            } else if is_string(index_type, "SPATIAL") {
                // the SUB_PART column may contain a non-null value for spatial indexes,
                // but this is not the prefix length
                length = None;
                IndexType::Spatial
            } else if cell(&row, 3).is_some_and(Value::is_truthy) {
                IndexType::Regular
            } else {
                IndexType::Unique
            };

            rows.push(IndexColumnMetadataRow::new(
                None,
                &cell_text(&row, 0)?,
                &cell_text(&row, 1)?,
                type_,
                false,
                None,
                &cell_text(&row, 4)?,
                length,
            ));
        }

        Ok(rows)
    }

    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-table-constraints-table.html>
    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-key-column-usage-table.html>
    async fn get_primary_key_constraint_columns(
        &self,
        table_name: Option<&str>,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        let sql = Self::constraint_columns_sql("PRIMARY KEY", table_name.is_some());

        self.fetch_all_numeric(&sql, &self.constraint_params(table_name))
            .await?
            .iter()
            .map(|row| {
                Ok(PrimaryKeyConstraintColumnRow::new(
                    None,
                    &cell_text(row, 0)?,
                    Some(&cell_text(row, 1)?),
                    true,
                    &cell_text(row, 2)?,
                ))
            })
            .collect()
    }

    async fn get_unique_constraint_columns(
        &self,
        table_name: Option<&str>,
    ) -> Rows<UniqueConstraintColumnMetadataRow> {
        let sql = Self::constraint_columns_sql("UNIQUE", table_name.is_some());

        self.fetch_all_numeric(&sql, &self.constraint_params(table_name))
            .await?
            .iter()
            .map(|row| {
                UniqueConstraintColumnMetadataRow::new(
                    None,
                    &cell_text(row, 0)?,
                    None,
                    Some(&cell_text(row, 1)?),
                    &cell_text(row, 2)?,
                )
            })
            .collect()
    }

    fn constraint_params<'a>(&'a self, table_name: Option<&'a str>) -> Vec<&'a str> {
        let mut params = vec![self.database_name.as_str(), self.database_name.as_str()];
        if let Some(table_name) = table_name {
            params.push(table_name);
        }
        params
    }

    /// The SQL of `getPrimaryKeyConstraintColumns()` (`PRIMARY KEY`) and
    /// `getUniqueConstraintColumns()` (`UNIQUE`).
    fn constraint_columns_sql(constraint_type: &str, include_table_name: bool) -> String {
        // The schema name is passed multiple times in the WHERE clause instead of using a JOIN condition to avoid
        // performance issues on MySQL older than 8.0 and the corresponding MariaDB versions caused by
        // https://bugs.mysql.com/bug.php?id=81347
        let mut conditions = vec!["tc.TABLE_SCHEMA = ?", "kcu.TABLE_SCHEMA = ?"];

        if include_table_name {
            conditions.push("tc.TABLE_NAME = ?");
        }

        let order_by = if constraint_type == "UNIQUE" {
            "ORDER BY tc.TABLE_NAME,\n    tc.CONSTRAINT_NAME,\n    kcu.ORDINAL_POSITION"
        } else {
            "ORDER BY TABLE_NAME,\n    kcu.ORDINAL_POSITION"
        };

        // On MySQL < 8.0 and MariaDB, information_schema compares object names case-insensitively and so
        // conflates table names that differ only in case. The binary comparison prevents that.
        format!(
            "SELECT tc.TABLE_NAME,
       tc.CONSTRAINT_NAME,
       kcu.COLUMN_NAME
FROM information_schema.TABLE_CONSTRAINTS tc
         INNER JOIN information_schema.KEY_COLUMN_USAGE kcu
                    ON kcu.TABLE_NAME = tc.TABLE_NAME
                        AND CONVERT(kcu.TABLE_NAME USING binary) = CONVERT(tc.TABLE_NAME USING binary)
                        AND kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME
WHERE {}
  AND tc.CONSTRAINT_TYPE = '{constraint_type}'
{order_by}",
            conditions.join(" AND "),
        )
    }

    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-key-column-usage-table.html>
    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-referential-constraints-table.html>
    async fn get_foreign_key_constraint_columns(
        &self,
        table_name: Option<&str>,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        // The schema name is passed multiple times in the WHERE clause instead of using a JOIN condition
        // to avoid performance issues on MySQL older than 8.0 and the corresponding MariaDB versions caused by
        // https://bugs.mysql.com/bug.php?id=81347
        let mut conditions = vec!["k.TABLE_SCHEMA = ?", "c.CONSTRAINT_SCHEMA = ?"];
        let mut params = vec![self.database_name.as_str(), self.database_name.as_str()];

        if let Some(table_name) = table_name {
            conditions.push("k.TABLE_NAME = ?");
            params.push(table_name);
        }

        // On MySQL < 8.0 and MariaDB, information_schema compares object names case-insensitively and so
        // conflates table names that differ only in case. The binary comparison prevents that.
        let sql = format!(
            "SELECT k.TABLE_NAME,
       k.CONSTRAINT_NAME,
       k.REFERENCED_TABLE_NAME,
       c.UPDATE_RULE,
       c.DELETE_RULE,
       k.COLUMN_NAME,
       k.REFERENCED_COLUMN_NAME
FROM information_schema.KEY_COLUMN_USAGE k
         INNER JOIN information_schema.REFERENTIAL_CONSTRAINTS c
                    ON c.CONSTRAINT_NAME = k.CONSTRAINT_NAME
                        AND c.TABLE_NAME = k.TABLE_NAME
                        AND CONVERT(c.TABLE_NAME USING binary) = CONVERT(k.TABLE_NAME USING binary)
WHERE {}
  AND k.REFERENCED_COLUMN_NAME IS NOT NULL
ORDER BY k.TABLE_NAME,
         k.CONSTRAINT_NAME,
         k.ORDINAL_POSITION",
            conditions.join(" AND "),
        );

        let mut rows = Vec::new();
        for row in self.fetch_all_numeric(&sql, &params).await? {
            let name = cell_text(&row, 1)?;
            rows.push(ForeignKeyConstraintColumnMetadataRow::new(
                ForeignKeyConstraintColumnMetadataRowParts {
                    referencing_schema_name: None,
                    referencing_table_name: &cell_text(&row, 0)?,
                    id: None,
                    name: Some(&name),
                    referenced_schema_name: None,
                    referenced_table_name: &cell_text(&row, 2)?,
                    match_type: MatchType::Simple,
                    on_update_action: create_referential_action(&cell_text(&row, 3)?)?,
                    on_delete_action: create_referential_action(&cell_text(&row, 4)?)?,
                    is_deferrable: false,
                    is_deferred: false,
                    referencing_column_name: &cell_text(&row, 5)?,
                    referenced_column_name: &cell_text(&row, 6)?,
                },
            )?);
        }

        Ok(rows)
    }

    async fn get_table_options(&self, table_name: Option<&str>) -> Rows<TableMetadataRow> {
        let sql = self
            .platform
            .fetch_table_options_by_table(table_name.is_some());

        let mut params = vec![self.database_name.as_str()];
        if let Some(table_name) = table_name {
            params.push(table_name);
        }

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| {
                let mut options = PhpArray::new();
                options.insert("engine", cell_value(row, 1));
                options.insert("autoincrement", cell_value(row, 2));
                options.insert("comment", cell_value(row, 3));
                options.insert(
                    "create_options",
                    Value::Array(parse_create_options(
                        nullable_text(cell(row, 4))?.as_deref(),
                    )),
                );
                options.insert("collation", cell_value(row, 5));
                options.insert("charset", cell_value(row, 6));
                Ok(TableMetadataRow::new(None, &cell_text(row, 0)?, options))
            })
            .collect()
    }
}

/// `createReferentialAction($value)`.
fn create_referential_action(value: &str) -> Result<ReferentialAction, Exception> {
    ReferentialAction::try_from_value(value).ok_or_else(|| assertion_error("$action !== null"))
}

impl MetadataProvider for MySQLMetadataProvider<'_> {
    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-schemata-table.html>
    async fn get_all_database_names(&self) -> Rows<DatabaseMetadataRow> {
        let sql = "SELECT SCHEMA_NAME
FROM information_schema.SCHEMATA
ORDER BY SCHEMA_NAME";

        self.connection
            .fetch_first_column(sql, &PhpArray::new(), &ParameterTypes::new())
            .await?
            .iter()
            .map(|database_name| Ok(DatabaseMetadataRow::new(&text_value(Some(database_name))?)))
            .collect()
    }

    async fn get_all_schema_names(&self) -> Rows<SchemaMetadataRow> {
        Err(NotSupported::new(&method("getAllSchemaNames")).into())
    }

    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-tables-table.html>
    async fn get_all_table_names(&self) -> Rows<TableMetadataRow> {
        let sql = "SELECT TABLE_NAME
FROM information_schema.TABLES
WHERE TABLE_SCHEMA = ?
  AND TABLE_TYPE = 'BASE TABLE'
ORDER BY TABLE_NAME";

        self.fetch_all_numeric(sql, &[self.database_name.as_str()])
            .await?
            .iter()
            .map(|row| {
                Ok(TableMetadataRow::new(
                    None,
                    &cell_text(row, 0)?,
                    PhpArray::new(),
                ))
            })
            .collect()
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
        self.get_table_options(None).await
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
        self.get_table_options(Some(table_name)).await
    }

    /// @link <https://dev.mysql.com/doc/refman/8.4/en/information-schema-views-table.html>
    async fn get_all_views(&self) -> Rows<ViewMetadataRow> {
        let sql = "SELECT TABLE_NAME,
       VIEW_DEFINITION
FROM information_schema.VIEWS
WHERE TABLE_SCHEMA = ?
ORDER BY TABLE_NAME";

        self.fetch_all_numeric(sql, &[self.database_name.as_str()])
            .await?
            .iter()
            .map(|row| {
                Ok(ViewMetadataRow::new(
                    None,
                    &cell_text(row, 0)?,
                    &cell_text(row, 1)?,
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
