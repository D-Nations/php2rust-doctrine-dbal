//! Port of Doctrine\DBAL\Schema\MySQLSchemaManager.php: the schema
//! manager for the `MySQL` RDBMS (and `MariaDB`).
//!
//! `createComparator()` (with and without a config) and `migrateSchema()`
//! are inherent `async` methods, as the trait does not carry the
//! comparator and the comparator needs the database's default table
//! options. The platform is a `&dyn AbstractMySQLPlatform`. A connection's
//! schema manager is still `SQLite`'s (`createSchemaManager()` names that
//! type): make this one with [`MySQLSchemaManager::new()`], as
//! `new MySQLSchemaManager($connection, $platform)` does.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/MySQLSchemaManager.php

use std::cell::OnceCell;

use php_value::{Key, PhpArray, Value};

use crate::exception::Exception;
use crate::platforms::mysql::{
    CachingCharsetMetadataProvider, CachingCollationMetadataProvider,
    Comparator as MySQLComparator, ConnectionCharsetMetadataProvider,
    ConnectionCollationMetadataProvider, DefaultTableOptions, MARIADB_ESCAPE_SEQUENCES,
    MySQLMetadataProvider, parse_create_options, parse_enum_expression, strtr,
};
use crate::platforms::sqlite::preg::{group, preg_match};
use crate::platforms::sqlite::{assertion_error, nullable_text, text_value};
use crate::platforms::{
    AbstractMySQLPlatform, AbstractPlatform, LENGTH_LIMIT_BLOB, LENGTH_LIMIT_MEDIUMBLOB,
    LENGTH_LIMIT_MEDIUMTEXT, LENGTH_LIMIT_TEXT, LENGTH_LIMIT_TINYBLOB, LENGTH_LIMIT_TINYTEXT,
};
use crate::{Connection, ParameterTypes};

use super::abstract_schema_manager::{
    AbstractSchemaManager, Row, field, field_text, isset, portable_table_indexes_list,
};
use super::array_map::ArrayMap;
use super::column::Column;
use super::comparator_config::ComparatorConfig;
use super::default_expression::{
    CurrentDate, CurrentTime, CurrentTimestamp, DefaultExpressionObject,
};
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::Index;
use super::schema::Schema;
use super::view::View;

type R<T> = Result<T, Exception>;

/// Schema manager for the `MySQL` RDBMS.
pub struct MySQLSchemaManager<'c> {
    connection: &'c Connection,
    platform: &'c dyn AbstractMySQLPlatform,
    default_table_options: OnceCell<DefaultTableOptions>,
}

impl std::fmt::Debug for MySQLSchemaManager<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MySQLSchemaManager")
            .field("platform", &self.platform.class())
            .finish_non_exhaustive()
    }
}

/// `$row[$key]` or `null`.
fn value(row: &Row, key: &str) -> Value {
    field(row, key).cloned().unwrap_or(Value::Null)
}

/// `array_change_key_case($row, CASE_LOWER)`.
fn lower_keys(row: &Row) -> Row {
    let mut lowered = PhpArray::new();
    for (key, value) in row.iter() {
        let key = match key {
            Key::Str(s) => Key::from_bytes(&s.as_bytes().to_ascii_lowercase()),
            other @ Key::Int(_) => other.clone(),
        };
        lowered.insert(key, value.clone());
    }
    lowered
}

/// `str_contains($row[$key], $needle)`.
fn contains(row: &Row, key: &str, needle: &str) -> R<bool> {
    Ok(field_text(row, key)?.contains(needle))
}

/// `(int) $value`.
fn int(value: Option<&Value>) -> i64 {
    value.map_or(0, php_value::ops::to_int)
}

/// `$value === $s`.
fn is_string(value: Option<&Value>, s: &str) -> bool {
    matches!(value, Some(Value::String(v)) if v.as_bytes() == s.as_bytes())
}

/// @link <https://dev.mysql.com/doc/refman/8.4/en/timestamp-initialization.html>
fn parse_mysql_column_default(type_: &str, default: &str) -> Value {
    // There is no way to tell whether the "CURRENT_TIMESTAMP" value represents a string or an expression in MySQL
    // schema introspection results. We rely on the fact that the "CURRENT_TIMESTAMP" expression is only supported
    // for the "DATETIME" and "TIMESTAMP" data types and represent this value as an expression only in these cases.
    if (type_ == "datetime" || type_ == "timestamp") && default == "CURRENT_TIMESTAMP" {
        return DefaultExpressionObject::value(CurrentTimestamp);
    }

    Value::string(default)
}

/// Return Doctrine/Mysql-compatible column default values for `MariaDB`
/// 10.2.7+ servers.
///
/// - Since `MariaDb` 10.2.7 column defaults stored in `information_schema`
///   are now quoted to distinguish them from expressions (see MDEV-10134).
/// - `CURRENT_TIMESTAMP`, `CURRENT_TIME`, `CURRENT_DATE` are stored in
///   `information_schema` as `current_timestamp()`, `currdate()`,
///   `currtime()`
/// - Quoted 'NULL' is not enforced by Maria, it is technically possible
///   to have null in some circumstances (see
///   <https://jira.mariadb.org/browse/MDEV-14053>)
/// - \' is always stored as '' in `information_schema` (normalized)
///
/// @link <https://mariadb.com/kb/en/library/information-schema-columns-table/>
/// @link <https://jira.mariadb.org/browse/MDEV-13132>
fn parse_mariadb_column_default(column_default: &str) -> R<Value> {
    if column_default == "NULL" {
        return Ok(Value::Null);
    }

    if let Some(matches) = preg_match(r"/^'(.*)'$/", column_default)? {
        return Ok(Value::string(strtr(
            group(&matches, 1).unwrap_or(""),
            MARIADB_ESCAPE_SEQUENCES,
        )));
    }

    Ok(match column_default {
        "current_timestamp()" => DefaultExpressionObject::value(CurrentTimestamp),
        "curdate()" => DefaultExpressionObject::value(CurrentDate),
        "curtime()" => DefaultExpressionObject::value(CurrentTime),
        default => Value::string(default),
    })
}

impl<'c> MySQLSchemaManager<'c> {
    /// `new MySQLSchemaManager($connection, $platform)`.
    pub fn new(connection: &'c Connection, platform: &'c dyn AbstractMySQLPlatform) -> Self {
        Self {
            connection,
            platform,
            default_table_options: OnceCell::new(),
        }
    }

    /// `createComparator()`.
    pub async fn create_comparator(&self) -> R<MySQLComparator<'c>> {
        self.create_comparator_with(ComparatorConfig::default())
            .await
    }

    /// `createComparator($config)`.
    pub async fn create_comparator_with(&self, config: ComparatorConfig) -> R<MySQLComparator<'c>> {
        let platform: &'c dyn AbstractPlatform = self.platform;
        Ok(MySQLComparator::new(
            platform,
            Box::new(CachingCharsetMetadataProvider::new(Box::new(
                ConnectionCharsetMetadataProvider::new(self.connection),
            ))),
            Box::new(CachingCollationMetadataProvider::new(Box::new(
                ConnectionCollationMetadataProvider::new(self.connection),
            ))),
            self.get_default_table_options().await?,
            config,
        ))
    }

    /// `migrateSchema($newSchema)`.
    pub async fn migrate_schema(&self, new_schema: &Schema) -> R<()> {
        let schema_diff = self
            .create_comparator()
            .await?
            .compare_schemas(&self.introspect_schema().await?, new_schema)?;

        self.alter_schema(&schema_diff).await
    }

    /// `getDefaultTableOptions()` (private): the database's character set
    /// and collation, read once.
    async fn get_default_table_options(&self) -> R<DefaultTableOptions> {
        if let Some(options) = self.default_table_options.get() {
            return Ok(options.clone());
        }

        let row = self
            .connection
            .fetch_numeric(
                "SELECT @@character_set_database, @@collation_database",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?
            .ok_or_else(|| assertion_error("$row !== false"))?;

        let options = DefaultTableOptions::new(
            text_value(row.get(&Key::Int(0)))?,
            text_value(row.get(&Key::Int(1)))?,
        );
        let _ = self.default_table_options.set(options.clone());
        Ok(options)
    }

    /// Returns the quoted representation of the given identifier name.
    fn get_quoted_identifier_name(&self, identifier: Option<&str>) -> Option<String> {
        identifier.map(|identifier| self.platform.quote_single_identifier(identifier))
    }

    async fn query(&self, sql: &str, params: Vec<&str>) -> R<crate::Result<'c>> {
        self.connection
            .execute_query(
                sql,
                &PhpArray::list(params.into_iter().map(Value::string)),
                &ParameterTypes::new(),
            )
            .await
    }
}

impl<'c> AbstractSchemaManager<'c> for MySQLSchemaManager<'c> {
    type MetadataProvider = MySQLMetadataProvider<'c>;

    fn connection(&self) -> &'c Connection {
        self.connection
    }

    fn platform(&self) -> &'c dyn AbstractPlatform {
        self.platform
    }

    async fn create_metadata_provider(&self) -> R<MySQLMetadataProvider<'c>> {
        MySQLMetadataProvider::new(self.connection, self.platform).await
    }

    /// `_getPortableTableDefinition($table)` (deprecated).
    fn get_portable_table_definition(&self, table: &Row) -> R<String> {
        field_text(table, "TABLE_NAME")
    }

    fn get_portable_view_definition(&self, view: &Row) -> R<View> {
        Ok(View::new(
            &field_text(view, "TABLE_NAME")?,
            &field_text(view, "VIEW_DEFINITION")?,
        ))
    }

    fn get_portable_table_indexes_list(
        &self,
        rows: &[Row],
        _table_name: &str,
    ) -> R<ArrayMap<Index>> {
        let mut portable = Vec::with_capacity(rows.len());
        for row in rows {
            let mut row = lower_keys(row);

            let primary = is_string(field(&row, "key_name"), "PRIMARY");
            row.insert("primary", Value::Bool(primary));

            if contains(&row, "index_type", "FULLTEXT")? {
                row.insert(
                    "flags",
                    Value::Array(PhpArray::list([Value::string("FULLTEXT")])),
                );
            } else if contains(&row, "index_type", "SPATIAL")? {
                row.insert(
                    "flags",
                    Value::Array(PhpArray::list([Value::string("SPATIAL")])),
                );
            }

            // Ignore prohibited prefix `length` for spatial index
            if !contains(&row, "index_type", "SPATIAL")? {
                let length = if isset(&row, "sub_part") {
                    Value::Int(int(field(&row, "sub_part")))
                } else {
                    Value::Null
                };
                row.insert("length", length);
            }

            portable.push(row);
        }

        portable_table_indexes_list(&portable)
    }

    fn get_portable_database_definition(&self, database: &Row) -> R<String> {
        field_text(database, "Database")
    }

    fn get_portable_table_column_definition(&self, table_column: &Row) -> R<Column> {
        let table_column = lower_keys(table_column);

        let db_type = field_text(&table_column, "type")?;
        let mut length = Value::Null;
        let mut scale = 0;
        let mut precision = Value::Null;
        let mut fixed = false;
        let mut values = PhpArray::new();

        let type_name = self.platform.doctrine_type_mapping(&db_type)?;

        match db_type.as_str() {
            "char" | "varchar" => {
                length = Value::Int(int(field(&table_column, "character_maximum_length")));
            }
            "binary" | "varbinary" => {
                length = Value::Int(int(field(&table_column, "character_octet_length")));
            }
            "tinytext" => length = Value::Int(LENGTH_LIMIT_TINYTEXT),
            "text" => length = Value::Int(LENGTH_LIMIT_TEXT),
            "mediumtext" => length = Value::Int(LENGTH_LIMIT_MEDIUMTEXT),
            "tinyblob" => length = Value::Int(LENGTH_LIMIT_TINYBLOB),
            "blob" => length = Value::Int(LENGTH_LIMIT_BLOB),
            "mediumblob" => length = Value::Int(LENGTH_LIMIT_MEDIUMBLOB),
            "float" | "double" | "real" | "numeric" | "decimal" => {
                precision = Value::Int(int(field(&table_column, "numeric_precision")));

                if isset(&table_column, "numeric_scale") {
                    scale = int(field(&table_column, "numeric_scale"));
                }
            }
            _ => {}
        }

        match db_type.as_str() {
            "char" | "binary" => fixed = true,
            "enum" => {
                values = PhpArray::list(
                    parse_enum_expression(&field_text(&table_column, "column_type")?)?
                        .into_iter()
                        .map(Value::string),
                );
            }
            _ => {}
        }

        let column_default = match nullable_text(field(&table_column, "default"))? {
            None => Value::Null,
            Some(default) if self.platform.is_mariadb() => parse_mariadb_column_default(&default)?,
            Some(default) => parse_mysql_column_default(&db_type, &default),
        };

        let mut options = PhpArray::new();
        options.insert("length", length);
        options.insert(
            "unsigned",
            Value::Bool(contains(&table_column, "column_type", "unsigned")?),
        );
        options.insert("fixed", Value::Bool(fixed));
        options.insert("default", column_default);
        options.insert(
            "notnull",
            Value::Bool(!is_string(field(&table_column, "null"), "YES")),
        );
        options.insert("scale", Value::Int(scale));
        options.insert("precision", precision);
        options.insert(
            "autoincrement",
            Value::Bool(contains(&table_column, "extra", "auto_increment")?),
        );
        options.insert("values", Value::Array(values));

        if !value(&table_column, "comment").is_null() {
            options.insert("comment", value(&table_column, "comment"));
        }

        let mut column = Column::new(
            &field_text(&table_column, "field")?,
            type_name.as_str(),
            &options,
        )?;
        column.set_platform_option("charset", value(&table_column, "characterset"));
        column.set_platform_option("collation", value(&table_column, "collation"));

        Ok(column)
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn get_portable_table_foreign_keys_list(
        &self,
        rows: &[Row],
    ) -> R<Vec<ForeignKeyConstraint>> {
        let mut list: ArrayMap<Row> = ArrayMap::new();
        for row in rows {
            let row = lower_keys(row);
            let constraint_name = field_text(&row, "constraint_name")?;
            if !list.contains_key(&constraint_name) {
                let rule = |key: &str| {
                    if !isset(&row, key) || is_string(field(&row, key), "RESTRICT") {
                        Value::Null
                    } else {
                        value(&row, key)
                    }
                };

                let mut definition = PhpArray::new();
                definition.insert(
                    "name",
                    self.get_quoted_identifier_name(Some(&constraint_name))
                        .map_or(Value::Null, Value::string),
                );
                definition.insert("local", Value::Array(PhpArray::new()));
                definition.insert("foreign", Value::Array(PhpArray::new()));
                definition.insert("foreignTable", value(&row, "referenced_table_name"));
                definition.insert("onDelete", rule("delete_rule"));
                definition.insert("onUpdate", rule("update_rule"));
                list.insert(constraint_name.clone(), definition);
            }

            if let Some(definition) = list.get_mut(&constraint_name) {
                for (list_key, row_key) in [
                    ("local", "column_name"),
                    ("foreign", "referenced_column_name"),
                ] {
                    if let Some(Value::Array(columns)) = definition.get_mut(&Key::from(list_key)) {
                        let _ = columns.push(value(&row, row_key));
                    }
                }
            }
        }

        list.values()
            .map(|definition| self.get_portable_table_foreign_key_definition(definition))
            .collect()
    }

    fn get_portable_table_foreign_key_definition(
        &self,
        table_foreign_key: &Row,
    ) -> R<ForeignKeyConstraint> {
        let names = |key: &str| -> R<Vec<String>> {
            match field(table_foreign_key, key) {
                Some(Value::Array(names)) => {
                    names.values().map(|name| text_value(Some(name))).collect()
                }
                _ => Ok(Vec::new()),
            }
        };

        let mut options = PhpArray::new();
        options.insert("onDelete", value(table_foreign_key, "onDelete"));
        options.insert("onUpdate", value(table_foreign_key, "onUpdate"));

        ForeignKeyConstraint::new(
            names("local")?,
            &field_text(table_foreign_key, "foreignTable")?,
            names("foreign")?,
            &nullable_text(field(table_foreign_key, "name"))?.unwrap_or_default(),
            options,
        )
    }

    async fn select_table_names(&self, database_name: &str) -> R<crate::Result<'c>> {
        let sql = "SELECT TABLE_NAME
FROM information_schema.TABLES
WHERE TABLE_SCHEMA = ?
  AND TABLE_TYPE = 'BASE TABLE'
ORDER BY TABLE_NAME";

        self.query(sql, vec![database_name]).await
    }

    async fn select_table_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        // The schema name is passed multiple times as a literal in the WHERE clause instead of using a JOIN condition
        // in order to avoid performance issues on MySQL older than 8.0 and the corresponding MariaDB versions
        // caused by https://bugs.mysql.com/bug.php?id=81347
        let mut conditions = vec!["c.TABLE_SCHEMA = ?", "t.TABLE_SCHEMA = ?"];
        let mut params = vec![database_name, database_name];

        if let Some(table_name) = table_name {
            conditions.push("t.TABLE_NAME = ?");
            params.push(table_name);
        }

        let sql = format!(
            "SELECT
       c.TABLE_NAME,
       c.COLUMN_NAME        AS field,
       {}                   AS type,
       c.COLUMN_TYPE,
       c.CHARACTER_MAXIMUM_LENGTH,
       c.CHARACTER_OCTET_LENGTH,
       c.NUMERIC_PRECISION,
       c.NUMERIC_SCALE,
       c.IS_NULLABLE        AS `null`,
       c.COLUMN_KEY         AS `key`,
       c.COLUMN_DEFAULT     AS `default`,
       c.EXTRA,
       c.COLUMN_COMMENT     AS comment,
       c.CHARACTER_SET_NAME AS characterset,
       c.COLLATION_NAME     AS collation
FROM information_schema.COLUMNS c
    INNER JOIN information_schema.TABLES t
        ON t.TABLE_NAME = c.TABLE_NAME
 WHERE {}
   AND t.TABLE_TYPE = 'BASE TABLE'
ORDER BY c.TABLE_NAME,
         c.ORDINAL_POSITION",
            self.platform
                .get_column_type_sql_snippet("c", database_name),
            conditions.join(" AND "),
        );

        self.query(&sql, params).await
    }

    async fn select_index_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let mut conditions = vec!["TABLE_SCHEMA = ?"];
        let mut params = vec![database_name];

        if let Some(table_name) = table_name {
            conditions.push("TABLE_NAME = ?");
            params.push(table_name);
        }

        let sql = format!(
            "SELECT
        TABLE_NAME,
        NON_UNIQUE  AS Non_Unique,
        INDEX_NAME  AS Key_name,
        COLUMN_NAME AS Column_Name,
        SUB_PART    AS Sub_Part,
        INDEX_TYPE  AS Index_Type
FROM information_schema.STATISTICS
WHERE {}
ORDER BY TABLE_NAME,
         INDEX_NAME,
         SEQ_IN_INDEX",
            conditions.join(" AND "),
        );

        self.query(&sql, params).await
    }

    async fn select_foreign_key_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        // The schema name is passed multiple times in the WHERE clause instead of using a JOIN condition
        // in order to avoid performance issues on MySQL older than 8.0 and the corresponding MariaDB versions
        // caused by https://bugs.mysql.com/bug.php?id=81347
        let mut conditions = vec!["k.TABLE_SCHEMA = ?", "c.CONSTRAINT_SCHEMA = ?"];
        let mut params = vec![database_name, database_name];

        if let Some(table_name) = table_name {
            conditions.push("k.TABLE_NAME = ?");
            params.push(table_name);
        }

        let sql = format!(
            "SELECT
            k.TABLE_NAME,
            k.CONSTRAINT_NAME,
            k.COLUMN_NAME,
            k.REFERENCED_TABLE_NAME,
            k.REFERENCED_COLUMN_NAME,
            k.ORDINAL_POSITION,
            c.UPDATE_RULE,
            c.DELETE_RULE
FROM information_schema.key_column_usage k
INNER JOIN information_schema.referential_constraints c
ON c.CONSTRAINT_NAME = k.CONSTRAINT_NAME
AND c.TABLE_NAME = k.TABLE_NAME
WHERE {}
AND k.REFERENCED_COLUMN_NAME IS NOT NULL
ORDER BY k.TABLE_NAME,
         k.CONSTRAINT_NAME,
         k.ORDINAL_POSITION",
            conditions.join(" AND "),
        );

        self.query(&sql, params).await
    }

    async fn fetch_table_options_by_table(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<ArrayMap<PhpArray>> {
        let sql = self
            .platform
            .fetch_table_options_by_table(table_name.is_some());

        let mut params = vec![database_name];
        if let Some(table_name) = table_name {
            params.push(table_name);
        }

        let metadata = self
            .query(&sql, params)
            .await?
            .fetch_all_associative_indexed()?;

        let mut table_options = ArrayMap::new();
        for (table, data) in metadata.iter() {
            let data = match data {
                Value::Array(data) => lower_keys(data),
                _ => PhpArray::new(),
            };

            let mut options = PhpArray::new();
            options.insert("engine", value(&data, "engine"));
            options.insert("collation", value(&data, "table_collation"));
            options.insert("charset", value(&data, "character_set_name"));
            options.insert("autoincrement", value(&data, "auto_increment"));
            options.insert("comment", value(&data, "table_comment"));
            options.insert(
                "create_options",
                Value::Array(parse_create_options(
                    nullable_text(field(&data, "create_options"))?.as_deref(),
                )),
            );

            let table = match table {
                Key::Int(i) => i.to_string(),
                Key::Str(s) => s.to_string_lossy().into_owned(),
            };
            table_options.insert(table, options);
        }

        Ok(table_options)
    }
}
