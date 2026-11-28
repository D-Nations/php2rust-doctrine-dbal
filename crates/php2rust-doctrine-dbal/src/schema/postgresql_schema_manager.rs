//! Port of Doctrine\DBAL\Schema\PostgreSQLSchemaManager.php: the schema
//! manager for the `PostgreSQL` RDBMS.
//!
//! `createComparator()` (with and without a config) and `migrateSchema()`
//! are inherent `async` methods, as for the other managers. PHP's
//! `getCurrentSchemaName()` caches the current schema; here the manager
//! caches it, and the methods that hand rows to
//! `_getPortableTableDefinition()` (which is not `async`) read it once
//! they have a row, where PHP reads it for the first row. The deprecated `getCurrentSchema()` and
//! `determineCurrentSchema()` are that cached value.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/PostgreSQLSchemaManager.php

use std::cell::OnceCell;

use php_value::{Key, PhpArray, Value};

use crate::exception::Exception;
use crate::platforms::postgresql::{
    Comparator as PostgreSQLComparator, PostgreSQLMetadataProvider, parse_column_type_parameters,
    parse_default_expression,
};
use crate::platforms::sqlite::preg::{group, preg_match};
use crate::platforms::sqlite::{assertion_error, nullable_text, text_value};
use crate::platforms::{AbstractPlatform, PostgreSQLPlatform};
use crate::{Connection, ParameterTypes};

use super::abstract_schema_manager::{
    AbstractSchemaManager, Row, field, field_text, isset, portable_table_indexes_list,
};
use super::array_map::ArrayMap;
use super::column::Column;
use super::comparator_config::ComparatorConfig;
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::Index;
use super::schema::Schema;
use super::sequence::Sequence;
use super::view::View;

type R<T> = Result<T, Exception>;

/// `PostgreSQL` Schema Manager.
pub struct PostgreSQLSchemaManager<'c> {
    connection: &'c Connection,
    platform: &'c PostgreSQLPlatform,
    current_schema_name: OnceCell<String>,
}

impl std::fmt::Debug for PostgreSQLSchemaManager<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgreSQLSchemaManager")
            .field("platform", &self.platform.class())
            .field("current_schema_name", &self.current_schema_name.get())
            .finish_non_exhaustive()
    }
}

/// `$row[$key]` or `null`.
fn value(row: &Row, key: &str) -> Value {
    field(row, key).cloned().unwrap_or(Value::Null)
}

/// `(bool) $row[$key]`.
fn truthy(row: &Row, key: &str) -> bool {
    field(row, key).is_some_and(Value::is_truthy)
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

/// `buildQueryConditions($tableName, $params)`: the conditions and the
/// parameters.
fn build_query_conditions(table_name: Option<&str>) -> (Vec<&'static str>, Vec<String>) {
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    if let Some(table_name) = table_name {
        let table_name = if table_name.contains('.') {
            // `[$schemaName, $tableName] = explode('.', $tableName)`
            let mut parts = table_name.split('.');
            let schema_name = parts.next().unwrap_or("");
            let table_name = parts.next().unwrap_or("");

            conditions.push("n.nspname = ?");
            params.push(schema_name.to_owned());
            table_name
        } else {
            conditions.push("n.nspname = ANY(current_schemas(false))");
            table_name
        };

        conditions.push("c.relname = ?");
        params.push(table_name.to_owned());
    }

    conditions.push("n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')");

    (conditions, params)
}

impl<'c> PostgreSQLSchemaManager<'c> {
    /// `new PostgreSQLSchemaManager($connection, $platform)`.
    pub fn new(connection: &'c Connection, platform: &'c PostgreSQLPlatform) -> Self {
        Self {
            connection,
            platform,
            current_schema_name: OnceCell::new(),
        }
    }

    /// `createComparator()`.
    pub fn create_comparator(&self) -> PostgreSQLComparator<'c> {
        self.create_comparator_with(ComparatorConfig::default())
    }

    /// `createComparator($config)`.
    pub fn create_comparator_with(&self, config: ComparatorConfig) -> PostgreSQLComparator<'c> {
        PostgreSQLComparator::new(self.platform, config)
    }

    /// `migrateSchema($newSchema)`.
    pub async fn migrate_schema(&self, new_schema: &Schema) -> R<()> {
        let schema_diff = self
            .create_comparator()
            .compare_schemas(&self.introspect_schema().await?, new_schema)?;

        self.alter_schema(&schema_diff).await
    }

    /// Determines the name of the current schema (deprecated
    /// `determineCurrentSchema()`), read once.
    pub async fn determine_current_schema(&self) -> R<String> {
        if let Some(name) = self.current_schema_name.get() {
            return Ok(name.clone());
        }

        let current_schema = self
            .connection
            .fetch_one(
                "SELECT current_schema()",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?;
        let current_schema = match current_schema {
            Some(Value::String(s)) => s.to_string_lossy().into_owned(),
            _ => return Err(assertion_error("is_string($currentSchema)")),
        };
        if current_schema.is_empty() {
            return Err(assertion_error("strlen($currentSchema) > 0"));
        }

        let _ = self.current_schema_name.set(current_schema.clone());
        Ok(current_schema)
    }

    /// Reads the current schema before `_getPortableTableDefinition()`
    /// first needs it (PHP reads it there, for the first row).
    async fn prime_current_schema(&self, rows: &[Row]) -> R<()> {
        if !rows.is_empty() {
            self.determine_current_schema().await?;
        }
        Ok(())
    }

    async fn query(&self, sql: &str, params: Vec<String>) -> R<crate::Result<'c>> {
        self.connection
            .execute_query(
                sql,
                &PhpArray::list(params.into_iter().map(Value::from)),
                &ParameterTypes::new(),
            )
            .await
    }
}

impl<'c> AbstractSchemaManager<'c> for PostgreSQLSchemaManager<'c> {
    type MetadataProvider = PostgreSQLMetadataProvider<'c>;

    fn connection(&self) -> &'c Connection {
        self.connection
    }

    fn platform(&self) -> &'c dyn AbstractPlatform {
        self.platform
    }

    #[allow(
        clippy::unused_async_trait_impl,
        reason = "preserves the async interface across implementations"
    )]
    async fn create_metadata_provider(&self) -> R<PostgreSQLMetadataProvider<'c>> {
        Ok(self.platform.create_metadata_provider(self.connection))
    }

    async fn list_schema_names(&self) -> R<Vec<String>> {
        self.connection
            .fetch_first_column(
                "SELECT schema_name
FROM   information_schema.schemata
WHERE  schema_name NOT LIKE 'pg\\_%'
AND    schema_name != 'information_schema'",
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?
            .iter()
            .map(|name| text_value(Some(name)))
            .collect()
    }

    /// Returns a list of all tables in the current database (deprecated).
    async fn list_table_names(&self) -> R<Vec<String>> {
        let database = self.get_database("listTableNames").await?;
        let rows = self
            .select_table_names(&database)
            .await?
            .fetch_all_associative()?;
        self.prime_current_schema(&rows).await?;

        let filter = self
            .connection
            .get_configuration()
            .get_schema_assets_filter()
            .clone();
        let mut names = Vec::new();
        for row in &rows {
            let name = self.get_portable_table_definition(row)?;
            if filter(&name) {
                names.push(name);
            }
        }
        Ok(names)
    }

    async fn fetch_table_columns_by_table(&self, database_name: &str) -> R<ArrayMap<Vec<Row>>> {
        let rows = self.fetch_table_columns(database_name, None).await?;
        self.prime_current_schema(&rows).await?;
        self.group_by_table(rows)
    }

    async fn fetch_index_columns_by_table(&self, database_name: &str) -> R<ArrayMap<Vec<Row>>> {
        let rows = self.fetch_index_columns(database_name, None).await?;
        self.prime_current_schema(&rows).await?;
        self.group_by_table(rows)
    }

    async fn fetch_foreign_key_columns_by_table(
        &self,
        database_name: &str,
    ) -> R<ArrayMap<Vec<Row>>> {
        let rows = self.fetch_foreign_key_columns(database_name, None).await?;
        self.prime_current_schema(&rows).await?;
        self.group_by_table(rows)
    }

    async fn determine_current_schema_name(&self) -> R<Option<String>> {
        Ok(Some(self.determine_current_schema().await?))
    }

    fn get_portable_table_foreign_key_definition(
        &self,
        table_foreign_key: &Row,
    ) -> R<ForeignKeyConstraint> {
        let condef = field_text(table_foreign_key, "condef")?;

        let mut on_update = Value::Null;
        let mut on_delete = Value::Null;

        if let Some(matches) = preg_match(
            "(ON UPDATE ([a-zA-Z0-9]+( (NULL|ACTION|DEFAULT))?))",
            &condef,
        )? {
            on_update = Value::string(group(&matches, 1).unwrap_or(""));
        }

        if let Some(matches) = preg_match(
            "(ON DELETE ([a-zA-Z0-9]+( (NULL|ACTION|DEFAULT))?))",
            &condef,
        )? {
            on_delete = Value::string(group(&matches, 1).unwrap_or(""));
        }

        let values = preg_match(r"/FOREIGN KEY \((.+)\) REFERENCES (.+)\((.+)\)/", &condef)?
            .ok_or_else(|| assertion_error("$result === 1"))?;

        // PostgreSQL returns identifiers that are keywords with quotes, we need them later, don't get
        // the idea to trim them here.
        let columns = |n: usize| -> Vec<String> {
            group(&values, n)
                .unwrap_or("")
                .split(',')
                .map(|c| {
                    c.trim_matches([' ', '\t', '\n', '\r', '\0', '\x0B'])
                        .to_owned()
                })
                .collect()
        };
        let local_columns = columns(1);
        let foreign_columns = columns(3);
        let foreign_table = group(&values, 2).unwrap_or("");

        let mut options = PhpArray::new();
        options.insert("onUpdate", on_update);
        options.insert("onDelete", on_delete);
        options.insert(
            "deferrable",
            Value::Bool(truthy(table_foreign_key, "condeferrable")),
        );
        options.insert(
            "deferred",
            Value::Bool(truthy(table_foreign_key, "condeferred")),
        );

        ForeignKeyConstraint::new(
            local_columns,
            foreign_table,
            foreign_columns,
            &field_text(table_foreign_key, "conname")?,
            options,
        )
    }

    fn get_portable_view_definition(&self, view: &Row) -> R<View> {
        Ok(View::new(
            &format!(
                "{}.{}",
                field_text(view, "schemaname")?,
                field_text(view, "viewname")?
            ),
            &field_text(view, "definition")?,
        ))
    }

    /// `_getPortableTableDefinition($table)` (deprecated): the table name,
    /// qualified unless it is in the current schema.
    fn get_portable_table_definition(&self, table: &Row) -> R<String> {
        let current_schema = self.current_schema_name.get();
        let schema_name = field_text(table, "schema_name")?;
        let table_name = field_text(table, "table_name")?;

        if matches!(field(table, "schema_name"), Some(Value::String(_)))
            && current_schema.is_some_and(|current| *current == schema_name)
        {
            return Ok(table_name);
        }

        Ok(format!("{schema_name}.{table_name}"))
    }

    fn get_portable_table_indexes_list(
        &self,
        rows: &[Row],
        _table_name: &str,
    ) -> R<ArrayMap<Index>> {
        let portable: Vec<Row> = rows
            .iter()
            .map(|row| {
                let mut portable = PhpArray::new();
                portable.insert("key_name", value(row, "relname"));
                portable.insert("non_unique", Value::Bool(!truthy(row, "indisunique")));
                portable.insert("primary", Value::Bool(truthy(row, "indisprimary")));
                portable.insert("where", value(row, "where"));
                portable.insert("column_name", value(row, "attname"));
                portable
            })
            .collect();

        portable_table_indexes_list(&portable)
    }

    fn get_portable_database_definition(&self, database: &Row) -> R<String> {
        field_text(database, "datname")
    }

    fn get_portable_sequence_definition(&self, sequence: &Row) -> R<Sequence> {
        let schema_name = field_text(sequence, "schemaname")?;
        let relname = field_text(sequence, "relname")?;

        let sequence_name = if schema_name == "public" {
            relname
        } else {
            format!("{schema_name}.{relname}")
        };

        let int = |key: &str| field(sequence, key).map_or(0, php_value::ops::to_int);

        Ok(Sequence::with_options(
            &sequence_name,
            int("increment_by"),
            int("min_value"),
            None,
        ))
    }

    fn get_portable_table_column_definition(&self, table_column: &Row) -> R<Column> {
        let table_column = lower_keys(table_column);

        let mut length = Value::Null;
        let mut precision = Value::Null;
        let mut scale = 0;
        let mut fixed = false;
        let mut jsonb = false;

        let mut db_type = field_text(&table_column, "type")?;

        let complete_type = if isset(&table_column, "domain_type")
            && !self.platform.has_doctrine_type_mapping_for(&db_type)?
        {
            db_type = field_text(&table_column, "domain_type")?;
            field_text(&table_column, "domain_complete_type")?
        } else {
            field_text(&table_column, "complete_type")?
        };

        let type_ = self.platform.get_doctrine_type_mapping(&db_type)?;

        match db_type.as_str() {
            "bpchar" | "varchar" => {
                let parameters = parse_column_type_parameters(&complete_type)?;
                if let Some(first) = parameters.first() {
                    length = Value::Int(*first);
                }
            }
            "double" | "decimal" | "money" | "numeric" => {
                let parameters = parse_column_type_parameters(&complete_type)?;
                if let Some(first) = parameters.first() {
                    precision = Value::Int(*first);
                }

                if let Some(second) = parameters.get(1) {
                    scale = *second;
                }
            }
            _ => {}
        }

        if db_type == "bpchar" {
            fixed = true;
        } else if db_type == "jsonb" {
            jsonb = true;
        }

        let mut options = PhpArray::new();
        options.insert("length", length);
        options.insert("notnull", Value::Bool(truthy(&table_column, "isnotnull")));
        options.insert(
            "default",
            parse_default_expression(nullable_text(field(&table_column, "default"))?.as_deref())?,
        );
        options.insert("precision", precision);
        options.insert("scale", Value::Int(scale));
        options.insert("fixed", Value::Bool(fixed));
        options.insert(
            "autoincrement",
            Value::Bool(matches!(
                field(&table_column, "attidentity"),
                Some(Value::String(s)) if s.as_bytes() == b"d"
            )),
        );

        if isset(&table_column, "comment") {
            options.insert("comment", value(&table_column, "comment"));
        }

        let mut column = Column::new(
            &field_text(&table_column, "field")?,
            type_.as_str(),
            &options,
        )?;

        if truthy(&table_column, "collation") {
            column.set_platform_option("collation", value(&table_column, "collation"));
        }

        // `Type::getType($column->getTypeName()) instanceof JsonType`
        let type_class = crate::types::get_type(column.get_type_name())
            .map_err(Exception::from)?
            .class()
            .to_owned();
        let is_json = type_class == "Doctrine\\DBAL\\Types\\JsonType"
            || type_class == "Doctrine\\DBAL\\Types\\JsonbType";
        if is_json {
            column.set_platform_option("jsonb", Value::Bool(jsonb));
        }

        Ok(column)
    }

    async fn select_table_names(&self, database_name: &str) -> R<crate::Result<'c>> {
        let sql = "SELECT quote_ident(table_name) AS table_name,
       table_schema AS schema_name
FROM information_schema.tables
WHERE table_catalog = ?
  AND table_schema NOT LIKE 'pg\\_%'
  AND table_schema != 'information_schema'
  AND table_name != 'geometry_columns'
  AND table_name != 'spatial_ref_sys'
  AND table_type = 'BASE TABLE'
ORDER BY
  quote_ident(table_name)";

        self.query(sql, vec![database_name.to_owned()]).await
    }

    async fn select_table_columns(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let (conditions, params) = build_query_conditions(table_name);

        let sql = format!(
            "SELECT quote_ident(n.nspname)               AS schema_name,
       quote_ident(c.relname)               AS table_name,
       quote_ident(a.attname)               AS field,
       t.typname                            AS type,
       format_type(a.atttypid, a.atttypmod) AS complete_type,
       bt.typname                           AS domain_type,
       format_type(bt.oid, t.typtypmod)     AS domain_complete_type,
       a.attnotnull                         AS isnotnull,
       a.attidentity,
       ({})                                 AS \"default\",
       dsc.description                      AS comment,
       CASE
           WHEN coll.collprovider = 'c'
               THEN coll.collcollate
           WHEN coll.collprovider = 'd'
               THEN NULL
           ELSE coll.collname
           END                              AS collation
FROM pg_attribute a
         JOIN pg_class c
              ON c.oid = a.attrelid
         JOIN pg_namespace n
              ON n.oid = c.relnamespace
         JOIN pg_type t
              ON t.oid = a.atttypid
         LEFT JOIN pg_type bt
                   ON t.typtype = 'd'
                       AND bt.oid = t.typbasetype
         LEFT JOIN pg_collation coll
                   ON coll.oid = a.attcollation
         LEFT JOIN pg_depend dep
                   ON dep.objid = c.oid
                       AND dep.deptype = 'e'
                       AND dep.classid = (SELECT oid FROM pg_class WHERE relname = 'pg_class')
         LEFT JOIN pg_description dsc
                   ON dsc.objoid = c.oid AND dsc.objsubid = a.attnum
         LEFT JOIN pg_inherits i
                   ON i.inhrelid = c.oid
         LEFT JOIN pg_class p
                   ON i.inhparent = p.oid
                       AND p.relkind = 'p'
WHERE {}
  -- 'r' for regular tables - 'p' for partitioned tables
  AND c.relkind IN ('r', 'p')
  AND a.attnum > 0
  AND dep.refobjid IS NULL
  -- exclude partitions (tables that inherit from partitioned tables)
  AND p.oid IS NULL
ORDER BY n.nspname,
    c.relname,
    a.attnum",
            self.platform.get_default_column_value_sql_snippet(),
            conditions.join(" AND "),
        );

        self.query(&sql, params).await
    }

    async fn select_index_columns(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let (conditions, params) = build_query_conditions(table_name);

        let sql = format!(
            "SELECT
       quote_ident(n.nspname) AS schema_name,
       quote_ident(c.relname) AS table_name,
       quote_ident(ic.relname) AS relname,
       i.indisunique,
       i.indisprimary,
       i.indkey,
       i.indrelid,
       pg_get_expr(indpred, indrelid) AS \"where\",
       quote_ident(attname) AS attname
  FROM pg_index i
       JOIN pg_class AS c ON c.oid = i.indrelid
       JOIN pg_namespace n ON n.oid = c.relnamespace
       JOIN pg_class AS ic ON ic.oid = i.indexrelid
       JOIN LATERAL UNNEST(i.indkey) WITH ORDINALITY AS keys(attnum, ord)
            ON TRUE
       JOIN pg_attribute a
            ON a.attrelid = c.oid
                AND a.attnum = keys.attnum
 WHERE {}
 ORDER BY 1, 2, keys.ord;",
            conditions.join(" AND "),
        );

        self.query(&sql, params).await
    }

    async fn select_foreign_key_columns(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<crate::Result<'c>> {
        let (conditions, params) = build_query_conditions(table_name);

        let sql = format!(
            "   SELECT
          quote_ident(tn.nspname) AS schema_name,
          quote_ident(tc.relname) AS table_name,
          quote_ident(r.conname) as conname,
          pg_get_constraintdef(r.oid, true) as condef,
          r.condeferrable,
          r.condeferred
          FROM pg_constraint r
              JOIN pg_class AS tc ON tc.oid = r.conrelid
              JOIN pg_namespace tn ON tn.oid = tc.relnamespace
          WHERE r.conrelid IN
          (
              SELECT c.oid
              FROM pg_class c
                JOIN pg_namespace n
                    ON n.oid = c.relnamespace
                WHERE {})
          AND r.contype = 'f'
          ORDER BY 1, 2",
            conditions.join(" AND "),
        );

        self.query(&sql, params).await
    }

    async fn fetch_table_options_by_table(
        &self,
        _database_name: &str,
        table_name: Option<&str>,
    ) -> R<ArrayMap<PhpArray>> {
        let (conditions, params) = build_query_conditions(table_name);

        let sql = format!(
            "SELECT quote_ident(n.nspname) AS schema_name,
       quote_ident(c.relname) AS table_name,
       CASE c.relpersistence WHEN 'u' THEN true ELSE false END as unlogged,
       obj_description(c.oid, 'pg_class') AS comment
FROM pg_class c
     INNER JOIN pg_namespace n
         ON n.oid = c.relnamespace
WHERE
      c.relkind = 'r'
  AND {}",
            conditions.join(" AND "),
        );

        let rows = self.query(&sql, params).await?.fetch_all_associative()?;
        self.prime_current_schema(&rows).await?;

        let mut table_options = ArrayMap::new();
        for row in rows {
            table_options.insert(self.get_portable_table_definition(&row)?, row);
        }

        Ok(table_options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_query_conditions() {
        assert_eq!(
            build_query_conditions(None),
            (
                vec!["n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')"],
                vec![]
            )
        );
        assert_eq!(
            build_query_conditions(Some("s.t")),
            (
                vec![
                    "n.nspname = ?",
                    "c.relname = ?",
                    "n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')"
                ],
                vec!["s".to_owned(), "t".to_owned()]
            )
        );
        assert_eq!(
            build_query_conditions(Some("t")).0[0],
            "n.nspname = ANY(current_schemas(false))"
        );
    }
}
