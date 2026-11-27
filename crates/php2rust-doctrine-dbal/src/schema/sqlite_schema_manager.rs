//! Port of Doctrine\DBAL\Schema\SQLiteSchemaManager.php: the `SQLite`
//! schema manager.
//!
//! `createComparator()` (with and without a config) and `migrateSchema()`
//! are inherent methods, as the trait does not carry the comparator. The regular expressions are
//! PHP's, run through php2rust-regex; a failing match is an exception
//! (PHP's warning and `false`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SQLiteSchemaManager.php

use php_value::{Key, PhpArray, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::sqlite::preg::{
    group, preg_match, preg_match_all, preg_replace, rtrim_newlines,
};
use crate::platforms::sqlite::{
    Comparator as SQLiteComparator, FOREIGN_KEY_DETAILS_PATTERN, SQLiteMetadataProvider,
    assertion_error, build_identifier_pattern, parse_without_rowid_from_sql, php_int, text_value,
};
use crate::platforms::{AbstractPlatform, SQLitePlatform};
use crate::types::Types;
use crate::{Connection, ParameterTypes};

use super::abstract_schema_manager::{AbstractSchemaManager, Row, field, field_text, isset};
use super::array_map::ArrayMap;
use super::column::Column;
use super::comparator_config::ComparatorConfig;
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::schema::Schema;
use super::table_diff::{TableDiff, TableDiffParts};
use super::view::View;

type R<T> = Result<T, Exception>;

const LINK_FK_SHORTHAND: &str = "https://github.com/doctrine/dbal/pull/6701";

/// `SQLite` `SchemaManager`.
#[derive(Debug, Clone, Copy)]
pub struct SQLiteSchemaManager<'c> {
    connection: &'c Connection,
    platform: &'c SQLitePlatform,
}

/// `$row[$key]` or `null`.
fn value(row: &Row, key: &str) -> Value {
    field(row, key).cloned().unwrap_or(Value::Null)
}

/// `$a === $b` for a string `$b`.
fn is_string_value(value: Option<&Value>, s: &str) -> bool {
    matches!(value, Some(Value::String(v)) if v.as_bytes() == s.as_bytes())
}

/// `strcasecmp($a, $b) === 0`.
fn equals_ignoring_case(a: &str, b: &str) -> bool {
    a.as_bytes().eq_ignore_ascii_case(b.as_bytes())
}

/// `array_merge($a, $b)` of two string-keyed rows.
fn merge(mut a: Row, b: &Row) -> Row {
    for (key, value) in b.iter() {
        a.insert(key.clone(), value.clone());
    }
    a
}

/// `getWhereClause($tableName, $params)`.
fn get_where_clause(table_name: Option<&str>, params: &mut Vec<Value>) -> String {
    let mut conditions = vec![
        "t.type = 'table'".to_owned(),
        "t.name NOT IN ('geometry_columns', 'spatial_ref_sys', 'sqlite_sequence')".to_owned(),
    ];

    if let Some(table_name) = table_name {
        conditions.push("t.name = ?".to_owned());
        params.push(Value::string(table_name));
    }

    conditions.join(" AND ")
}

struct ForeignKeyData {
    id: Value,
    name: Value,
    local: Vec<String>,
    foreign: Vec<String>,
    foreign_table: String,
    on_delete: Value,
    on_update: Value,
    deferrable: Value,
    deferred: Value,
}

impl<'c> SQLiteSchemaManager<'c> {
    /// `new SQLiteSchemaManager($connection, $platform)`.
    pub fn new(connection: &'c Connection, platform: &'c SQLitePlatform) -> Self {
        Self {
            connection,
            platform,
        }
    }

    /// `createComparator()`.
    pub fn create_comparator(&self) -> SQLiteComparator<'c> {
        SQLiteComparator::new(self.platform, ComparatorConfig::default())
    }

    /// `createComparator($config)`.
    pub fn create_comparator_with(&self, config: ComparatorConfig) -> SQLiteComparator<'c> {
        SQLiteComparator::new(self.platform, config)
    }

    /// `migrateSchema($newSchema)`.
    pub async fn migrate_schema(&self, new_schema: &Schema) -> R<()> {
        let schema_diff = self
            .create_comparator()
            .compare_schemas(&self.introspect_schema().await?, new_schema)?;

        self.alter_schema(&schema_diff).await
    }

    fn parse_column_collation_from_sql(&self, column: &str, sql: &str) -> R<Option<String>> {
        let pattern = format!(
            "{{{}{}}}is",
            build_identifier_pattern(self.platform, column),
            r#"[^,(]+(?:\([^()]+\)[^,]*)?(?:(?:DEFAULT|CHECK)\s*(?:\(.*?\))?[^,]*)*COLLATE\s+["']?([^\s,"')]+)"#
        );

        let Some(matches) = preg_match(&pattern, sql)? else {
            return Ok(None);
        };

        Ok(Some(group(&matches, 1).unwrap_or("").to_owned()))
    }

    fn parse_table_comment_from_sql(&self, table: &str, sql: &str) -> R<Option<String>> {
        let pattern = format!(
            "/\\s* # Allow whitespace characters at start of line\nCREATE\\sTABLE{}\n( # Start capture\n   (?:\\s*--[^\\n]*\\n?)+ # Capture anything that starts with whitespaces followed by -- until the end of the line(s)\n)/ix",
            build_identifier_pattern(self.platform, table)
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

    fn parse_column_comment_from_sql(&self, column: &str, sql: &str) -> R<String> {
        let pattern = format!(
            "{{[\\s(,]{}{}}}i",
            build_identifier_pattern(self.platform, column),
            r"(?:\([^)]*?\)|[^,(])*?,?((?:(?!\n))(?:\s*--[^\n]*\n?)+)"
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

    async fn get_create_table_sql(&self, table: &str) -> R<String> {
        let sql = self
            .connection
            .fetch_one(
                "SELECT sql\n  FROM (\n      SELECT *\n        FROM sqlite_master\n   UNION ALL\n      SELECT *\n        FROM sqlite_temp_master\n  )\nWHERE type = 'table'\nAND name = ?",
                &PhpArray::list([Value::string(table)]),
                &ParameterTypes::new(),
            )
            .await?;

        match sql {
            Some(sql) => text_value(Some(&sql)),
            None => Ok(String::new()),
        }
    }

    async fn get_foreign_key_details(&self, table: &str) -> R<Vec<Row>> {
        let create_sql = self.get_create_table_sql(table).await?;

        let matches = preg_match_all(FOREIGN_KEY_DETAILS_PATTERN, &create_sql)?;
        let count = matches.first().map_or(0, Vec::len);
        if count == 0 {
            return Ok(Vec::new());
        }

        let cell = |n: usize, i: usize| matches.get(n).and_then(|g| g.get(i)).cloned();

        let mut details = Vec::new();
        for i in 0..count {
            let mut detail = PhpArray::new();
            detail.insert(
                "constraint_name",
                Value::string(cell(1, i).unwrap_or_default()),
            );
            detail.insert(
                "deferrable",
                Value::Bool(cell(2, i).is_some_and(|d| equals_ignoring_case(&d, "deferrable"))),
            );
            detail.insert(
                "deferred",
                Value::Bool(cell(3, i).is_some_and(|d| equals_ignoring_case(&d, "deferred"))),
            );
            details.push(detail);
        }

        Ok(details)
    }

    /// Fetches names of primary key columns. If the table name is
    /// specified, narrows down the selection to this table.
    async fn fetch_primary_key_columns(&self, table_name: Option<&str>) -> R<Vec<Row>> {
        let mut params = Vec::new();

        let sql = format!(
            "    SELECT t.name AS table_name,\n           p.name\n      FROM sqlite_master t\n      JOIN pragma_table_info(t.name) p\n     WHERE {}\n       AND p.pk > 0\n  ORDER BY t.name,\n           p.pk",
            get_where_clause(table_name, &mut params)
        );

        self.connection
            .fetch_all_associative(&sql, &PhpArray::list(params), &ParameterTypes::new())
            .await
    }

    async fn query(&self, sql: &str, params: Vec<Value>) -> R<crate::Result<'c>> {
        self.connection
            .execute_query(sql, &PhpArray::list(params), &ParameterTypes::new())
            .await
    }
}

impl<'c> AbstractSchemaManager<'c> for SQLiteSchemaManager<'c> {
    type MetadataProvider = SQLiteMetadataProvider<'c>;

    fn connection(&self) -> &'c Connection {
        self.connection
    }

    fn platform(&self) -> &'c dyn AbstractPlatform {
        self.platform
    }

    async fn create_metadata_provider(&self) -> R<SQLiteMetadataProvider<'c>> {
        Ok(self.platform.create_metadata_provider(self.connection))
    }

    async fn create_foreign_key(&self, foreign_key: &ForeignKeyConstraint, table: &str) -> R<()> {
        let table = self.introspect_table(table).await?;

        self.alter_table(&TableDiff::new(
            table,
            TableDiffParts {
                added_foreign_keys: vec![foreign_key.clone()],
                ..TableDiffParts::default()
            },
        ))
        .await
    }

    async fn drop_foreign_key(&self, name: &str, table: &str) -> R<()> {
        let table = self.introspect_table(table).await?;

        let foreign_key = table.get_foreign_key(name)?.clone();

        self.alter_table(&TableDiff::new(
            table,
            TableDiffParts {
                dropped_foreign_keys: vec![foreign_key],
                ..TableDiffParts::default()
            },
        ))
        .await
    }

    fn get_portable_table_definition(&self, table: &Row) -> R<String> {
        field_text(table, "table_name")
    }

    fn get_portable_table_column_definition(&self, table_column: &Row) -> R<Column> {
        let type_ = field_text(table_column, "type")?;
        let Some(matches) = preg_match(r"/^([A-Z\s]+?)(?:\s*\((\d+)(?:,\s*(\d+))?\))?$/i", &type_)?
        else {
            return Err(assertion_error("$matchResult === 1"));
        };

        let mut db_type = group(&matches, 1).unwrap_or("").to_ascii_lowercase();

        let mut length = Value::Null;
        let mut precision = Value::Null;
        let mut fixed = false;
        let mut unsigned = false;
        let mut scale = 0;

        if let Some(m2) = group(&matches, 2) {
            if let Some(m3) = group(&matches, 3) {
                precision = Value::Int(php_int(m2));
                scale = php_int(m3);
            } else {
                length = Value::Int(php_int(m2));
            }
        }

        if db_type.contains(" unsigned") {
            db_type = db_type.replace(" unsigned", "");
            unsigned = true;
        }

        let type_name = self.platform.get_doctrine_type_mapping(&db_type)?;
        let mut default = value(table_column, "dflt_value");
        if is_string_value(Some(&default), "NULL") {
            default = Value::Null;
        }

        if let Value::String(s) = &default {
            let s = s.to_string_lossy().into_owned();
            // SQLite returns the default value as a literal expression, so
            // we need to parse it
            if let Some(matches) = preg_match(r"/^'(.*)'$/s", &s)? {
                default = Value::string(group(&matches, 1).unwrap_or("").replace("''", "'"));
            }
        }

        let notnull = field(table_column, "notnull").is_some_and(Value::is_truthy);

        if db_type == "char" {
            fixed = true;
        }

        let mut options = PhpArray::new();
        options.insert("autoincrement", value(table_column, "autoincrement"));
        options.insert("comment", value(table_column, "comment"));
        options.insert("length", length);
        options.insert("unsigned", Value::Bool(unsigned));
        options.insert("fixed", Value::Bool(fixed));
        options.insert("notnull", Value::Bool(notnull));
        options.insert("default", default);
        options.insert("precision", precision);
        options.insert("scale", Value::Int(scale));

        let mut column = Column::new(
            &field_text(table_column, "name")?,
            type_name.as_str(),
            &options,
        )?;

        if type_name == Types::STRING || type_name == Types::TEXT {
            let collation = if isset(table_column, "collation") {
                value(table_column, "collation")
            } else {
                Value::string("BINARY")
            };
            column.set_platform_option("collation", collation);
        }

        Ok(column)
    }

    fn get_portable_view_definition(&self, view: &Row) -> R<View> {
        Ok(View::new(
            &field_text(view, "name")?,
            &field_text(view, "sql")?,
        ))
    }

    async fn get_portable_table_foreign_keys_list(
        &self,
        rows: &[Row],
    ) -> R<Vec<ForeignKeyConstraint>> {
        let mut list: Vec<ForeignKeyData> = Vec::new();
        for row in rows {
            let mut lowered = PhpArray::new();
            for (key, value) in row.iter() {
                let key = match key {
                    Key::Str(s) => Key::from(s.to_string_lossy().to_ascii_lowercase().as_str()),
                    other @ Key::Int(_) => other.clone(),
                };
                lowered.insert(key, value.clone());
            }
            let row = lowered;
            let id = value(&row, "id");
            let position = list.iter().position(|data| data.id.identical(&id));
            let index = if let Some(index) = position {
                index
            } else {
                let referential = |key: &str| {
                    if !isset(&row, key) || is_string_value(field(&row, key), "RESTRICT") {
                        Value::Null
                    } else {
                        value(&row, key)
                    }
                };

                list.push(ForeignKeyData {
                    id: id.clone(),
                    name: value(&row, "constraint_name"),
                    local: Vec::new(),
                    foreign: Vec::new(),
                    foreign_table: field_text(&row, "table")?,
                    on_delete: referential("on_delete"),
                    on_update: referential("on_update"),
                    deferrable: value(&row, "deferrable"),
                    deferred: value(&row, "deferred"),
                });
                list.len() - 1
            };

            let local = field_text(&row, "from")?;
            let to = field(&row, "to").filter(|to| !to.is_null()).cloned();
            if let Some(data) = list.get_mut(index) {
                data.local.push(local);
                if let Some(to) = to {
                    data.foreign.push(text_value(Some(&to))?);
                }
            }
        }

        for data in &mut list {
            if !data.foreign.is_empty() {
                continue;
            }

            // Inferring a shorthand form for the foreign key constraint,
            // where the "to" field is empty.
            // @see https://www.sqlite.org/foreignkeys.html#fk_indexes.
            let foreign_table_primary_key_column_rows = self
                .fetch_primary_key_columns(Some(&data.foreign_table))
                .await?;

            if foreign_table_primary_key_column_rows.is_empty() {
                deprecations::trigger(LINK_FK_SHORTHAND, || {
                    "Introspection of SQLite foreign key constraints with omitted referenced column names in an incomplete schema is deprecated.".to_owned()
                });

                continue;
            }

            data.foreign = foreign_table_primary_key_column_rows
                .iter()
                .map(|row| field_text(row, "name"))
                .collect::<R<_>>()?;
        }

        let mut foreign_keys = Vec::new();
        for data in list {
            let mut row = PhpArray::new();
            row.insert("name", data.name);
            row.insert(
                "local",
                Value::Array(PhpArray::list(data.local.into_iter().map(Value::string))),
            );
            row.insert(
                "foreign",
                Value::Array(PhpArray::list(data.foreign.into_iter().map(Value::string))),
            );
            row.insert("foreignTable", Value::string(data.foreign_table));
            row.insert("onDelete", data.on_delete);
            row.insert("onUpdate", data.on_update);
            row.insert("deferrable", data.deferrable);
            row.insert("deferred", data.deferred);
            foreign_keys.push(self.get_portable_table_foreign_key_definition(&row)?);
        }
        Ok(foreign_keys)
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
        options.insert("deferrable", value(table_foreign_key, "deferrable"));
        options.insert("deferred", value(table_foreign_key, "deferred"));

        ForeignKeyConstraint::new(
            names("local")?,
            &field_text(table_foreign_key, "foreignTable")?,
            names("foreign")?,
            &field_text(table_foreign_key, "name")?,
            options,
        )
    }

    async fn select_table_names(&self, _database_name: &str) -> R<crate::Result<'c>> {
        self.query(
            "SELECT name AS table_name\nFROM sqlite_master\nWHERE type = 'table'\n  AND name NOT IN ('geometry_columns', 'spatial_ref_sys', 'sqlite_sequence')\nUNION ALL\nSELECT name\nFROM sqlite_temp_master\nWHERE type = 'table'\nORDER BY name",
            Vec::new(),
        )
        .await
    }

    async fn select_table_columns(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let mut params = Vec::new();

        let sql = format!(
            "            SELECT t.name AS table_name,\n                   c.*\n              FROM sqlite_master t\n              JOIN pragma_table_info(t.name) c\n             WHERE {}\n          ORDER BY t.name,\n                   c.cid",
            get_where_clause(table_name, &mut params)
        );

        self.query(&sql, params).await
    }

    /// @link <https://www.sqlite.org/pragma.html#pragma_index_info>
    /// @link <https://www.sqlite.org/pragma.html#pragma_table_info>
    /// @link <https://www.sqlite.org/fileformat2.html#internal_schema_objects>
    async fn select_index_columns(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let mut params = Vec::new();

        let sql = format!(
            "            SELECT t.name AS table_name,\n                   i.name,\n                   i.\"unique\",\n                   c.name AS column_name\n              FROM sqlite_master t\n              JOIN pragma_index_list(t.name) i\n              JOIN pragma_index_info(i.name) c\n             WHERE {}\n               AND i.name NOT LIKE 'sqlite_%'\n          ORDER BY t.name, i.seq, c.seqno",
            get_where_clause(table_name, &mut params)
        );

        self.query(&sql, params).await
    }

    async fn select_foreign_key_columns(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let mut params = Vec::new();

        let sql = format!(
            "            SELECT t.name AS table_name,\n                   p.*\n              FROM sqlite_master t\n              JOIN pragma_foreign_key_list(t.name) p\n                ON p.seq != '-1'\n             WHERE {}\n          ORDER BY t.name,\n                   p.id DESC,\n                   p.seq",
            get_where_clause(table_name, &mut params)
        );

        self.query(&sql, params).await
    }

    async fn fetch_table_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        let rows = self
            .select_table_columns(database_name, table_name)
            .await?
            .fetch_all_associative()?;

        let mut sql_by_table: ArrayMap<String> = ArrayMap::new();
        let mut pk_column_names_by_table: ArrayMap<Vec<String>> = ArrayMap::new();

        for row in &rows {
            let table_name = field_text(row, "table_name")?;

            if !sql_by_table.contains_key(&table_name) {
                let sql = self.get_create_table_sql(&table_name).await?;
                sql_by_table.insert(table_name.clone(), sql);
            }

            let pk = field(row, "pk");
            if !matches!(pk, Some(Value::Int(0))) && !is_string_value(pk, "0") {
                let name = field_text(row, "name")?;
                match pk_column_names_by_table.get_mut(&table_name) {
                    Some(names) => names.push(name),
                    None => pk_column_names_by_table.insert(table_name, vec![name]),
                }
            }
        }

        let mut result = Vec::new();
        for row in rows {
            let table_name = field_text(&row, "table_name")?;
            let column_name = field_text(&row, "name")?;
            let table_sql = sql_by_table.get(&table_name).cloned().unwrap_or_default();

            let autoincrement = pk_column_names_by_table
                .get(&table_name)
                .is_some_and(|names| names.len() == 1 && names[0] == column_name)
                && is_string_value(field(&row, "type"), "INTEGER");

            let mut extra = PhpArray::new();
            extra.insert("autoincrement", Value::Bool(autoincrement));
            extra.insert(
                "collation",
                self.parse_column_collation_from_sql(&column_name, &table_sql)?
                    .map_or(Value::Null, Value::string),
            );
            extra.insert(
                "comment",
                Value::string(self.parse_column_comment_from_sql(&column_name, &table_sql)?),
            );
            result.push(merge(row, &extra));
        }

        Ok(result)
    }

    async fn fetch_index_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        let mut result = Vec::new();

        let pk_column_name_rows = self.fetch_primary_key_columns(table_name).await?;

        for pk_column_name_row in &pk_column_name_rows {
            let mut row = PhpArray::new();
            row.insert("table_name", value(pk_column_name_row, "table_name"));
            row.insert("key_name", Value::string("primary"));
            row.insert("primary", Value::Bool(true));
            row.insert("non_unique", Value::Bool(false));
            row.insert("column_name", value(pk_column_name_row, "name"));
            result.push(row);
        }

        let index_column_rows = self
            .select_index_columns(database_name, table_name)
            .await?
            .fetch_all_associative()?;

        for index_column_row in &index_column_rows {
            let mut row = PhpArray::new();
            row.insert("table_name", value(index_column_row, "table_name"));
            row.insert("key_name", value(index_column_row, "name"));
            row.insert("primary", Value::Bool(false));
            row.insert(
                "non_unique",
                Value::Bool(!field(index_column_row, "unique").is_some_and(Value::is_truthy)),
            );
            row.insert("column_name", value(index_column_row, "column_name"));
            result.push(row);
        }

        Ok(result)
    }

    async fn fetch_foreign_key_columns(
        &self,
        database_name: &str,
        table_name: Option<&str>,
    ) -> R<Vec<Row>> {
        let mut columns_by_table: ArrayMap<Vec<Row>> = ArrayMap::new();
        for column in self
            .select_foreign_key_columns(database_name, table_name)
            .await?
            .fetch_all_associative()?
        {
            let table = field_text(&column, "table_name")?;
            match columns_by_table.get_mut(&table) {
                Some(columns) => columns.push(column),
                None => columns_by_table.insert(table, vec![column]),
            }
        }

        let mut columns = Vec::new();
        for (table, table_columns) in columns_by_table.iter() {
            let foreign_key_details = self.get_foreign_key_details(table).await?;
            let foreign_key_count = i64::try_from(foreign_key_details.len()).unwrap_or(i64::MAX);

            for column in table_columns {
                let id = match field(column, "id") {
                    Some(Value::Int(id)) => *id,
                    other => php_int(&text_value(other)?),
                };
                // SQLite identifies foreign keys in reverse order of
                // appearance in SQL
                let details = usize::try_from(foreign_key_count - id - 1)
                    .ok()
                    .and_then(|i| foreign_key_details.get(i))
                    .ok_or_else(|| {
                        Exception::new(
                            Exception::TYPE_ERROR,
                            "array_merge(): Argument #2 must be of type array, null given",
                        )
                    })?;
                columns.push(merge(column.clone(), details));
            }
        }

        Ok(columns)
    }

    async fn fetch_table_options_by_table(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<ArrayMap<PhpArray>> {
        let tables = match table_name {
            None => self.list_table_names().await?,
            Some(table_name) => vec![table_name.to_owned()],
        };

        let mut table_options: ArrayMap<PhpArray> = ArrayMap::new();
        for table in tables {
            let create_sql = self.get_create_table_sql(&table).await?;

            let mut options = PhpArray::new();

            if let Some(comment) = self.parse_table_comment_from_sql(&table, &create_sql)? {
                options.insert("comment", Value::string(comment));
            }

            if parse_without_rowid_from_sql(&create_sql)? {
                options.insert("without_rowid", Value::Bool(true));
            }

            if !options.is_empty() {
                match table_options.get_mut(&table) {
                    Some(existing) => {
                        for (key, value) in options.iter() {
                            existing.insert(key.clone(), value.clone());
                        }
                    }
                    None => table_options.insert(table, options),
                }
            }
        }

        Ok(table_options)
    }
}
