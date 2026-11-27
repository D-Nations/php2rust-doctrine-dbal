//! Port of Doctrine\DBAL\Platforms\PostgreSQL\PostgreSQLMetadataProvider.php:
//! the `PostgreSQL` metadata of databases, schemas, tables, columns,
//! indexes, constraints, views and sequences, read from the system
//! catalogs.
//!
//! PHP's generators are the rows collected into a `Vec`. PHP's `assert()`
//! calls (enabled in DBAL's test suite) fail with an `AssertionError`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/PostgreSQL/PostgreSQLMetadataProvider.php

use php_value::{PhpArray, Value};

use crate::exception::Exception;
use crate::platforms::sqlite::preg::{group, preg_match};
use crate::platforms::sqlite::{assertion_error, nullable_text, text_value};
use crate::platforms::{AbstractPlatform, PostgreSQLPlatform};
use crate::schema::exception::unsupported_name_from_null_schema_name;
use crate::schema::metadata::{
    DatabaseMetadataRow, ForeignKeyConstraintColumnMetadataRow,
    ForeignKeyConstraintColumnMetadataRowParts, IndexColumnMetadataRow, MetadataProvider,
    PrimaryKeyConstraintColumnRow, SchemaMetadataRow, SequenceMetadataRow, TableColumnMetadataRow,
    TableMetadataRow, UniqueConstraintColumnMetadataRow, ViewMetadataRow,
};
use crate::schema::{Column, IndexType, MatchType, ReferentialAction};
use crate::{Connection, ParameterTypes};

const CLASS: &str = "Doctrine\\DBAL\\Platforms\\PostgreSQL\\PostgreSQLMetadataProvider";

type Rows<T> = Result<Vec<T>, Exception>;

fn method(name: &str) -> String {
    format!("{CLASS}::{name}")
}

/// `self::REFERENTIAL_ACTIONS[$value]`.
///
/// @link <https://www.postgresql.org/docs/current/catalog-pg-constraint.html>
fn referential_action(value: &str) -> Result<ReferentialAction, Exception> {
    match value {
        "a" => Ok(ReferentialAction::NoAction),
        "c" => Ok(ReferentialAction::Cascade),
        "d" => Ok(ReferentialAction::SetDefault),
        "n" => Ok(ReferentialAction::SetNull),
        "r" => Ok(ReferentialAction::Restrict),
        _ => Err(assertion_error(&format!(
            "array_key_exists('{value}', self::REFERENTIAL_ACTIONS)"
        ))),
    }
}

/// `$row[$index]`.
fn cell(row: &PhpArray, index: i64) -> Option<&Value> {
    row.get(&index.into())
}

fn cell_text(row: &PhpArray, index: i64) -> Result<String, Exception> {
    text_value(cell(row, index))
}

fn cell_nullable(row: &PhpArray, index: i64) -> Result<Option<String>, Exception> {
    nullable_text(cell(row, index))
}

fn cell_value(row: &PhpArray, index: i64) -> Value {
    cell(row, index).cloned().unwrap_or(Value::Null)
}

/// `(bool) $row[$index]`.
fn cell_bool(row: &PhpArray, index: i64) -> bool {
    cell(row, index).is_some_and(Value::is_truthy)
}

/// `(int) $row[$index]`.
fn cell_int(row: &PhpArray, index: i64) -> i64 {
    cell(row, index).map_or(0, php_value::ops::to_int)
}

/// Parses the parameters between parenthesis in the data type.
pub(crate) fn parse_column_type_parameters(type_: &str) -> Result<Vec<i64>, Exception> {
    let Some(matches) = preg_match(r"/\((\d+)(?:,(\d+))?\)/", type_)? else {
        return Ok(Vec::new());
    };

    let to_int = |s: &str| php_value::ops::to_int(&Value::string(s));

    let mut parameters = vec![to_int(group(&matches, 1).unwrap_or(""))];

    if let Some(scale) = group(&matches, 2) {
        parameters.push(to_int(scale));
    }

    Ok(parameters)
}

/// Parses a default value expression as given by `PostgreSQL`.
pub(crate) fn parse_default_expression(expression: Option<&str>) -> Result<Value, Exception> {
    let Some(expression) = expression.filter(|e| !e.starts_with("NULL::")) else {
        return Ok(Value::Null);
    };

    if expression == "true" {
        return Ok(Value::Bool(true));
    }

    if expression == "false" {
        return Ok(Value::Bool(false));
    }

    if let Some(matches) = preg_match("/^'(.*)'::/s", expression)? {
        return Ok(Value::string(
            group(&matches, 1).unwrap_or("").replace("''", "'"),
        ));
    }

    Ok(Value::string(expression))
}

fn build_namespace_name_predicate(column_name: &str) -> String {
    format!("{column_name} NOT LIKE 'pg\\_%' AND {column_name} != 'information_schema'")
}

/// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
fn build_table_predicate(kind_column_name: &str, name_column_name: &str) -> String {
    // r = ordinary table, p = partitioned table,
    format!(
        "{kind_column_name} IN ('r', 'p') AND {name_column_name} NOT IN ('geometry_columns', 'spatial_ref_sys')"
    )
}

/// `buildTableQueryPredicate()`: the conditions and the parameters.
fn build_table_query_predicate(
    namespace_relation: &str,
    schema_name: Option<&str>,
    table_relation: &str,
    table_name: Option<&str>,
) -> Result<(String, Vec<String>), Exception> {
    let mut conditions = Vec::new();
    let mut params = Vec::new();

    if table_name.is_none() != schema_name.is_none() {
        return Err(assertion_error(
            "($tableName === null) === ($schemaName === null)",
        ));
    }

    if let (Some(table_name), Some(schema_name)) = (table_name, schema_name) {
        conditions.push(format!("{namespace_relation}.nspname = ?"));
        params.push(schema_name.to_owned());

        conditions.push(format!("{table_relation}.relname = ?"));
        params.push(table_name.to_owned());
    }

    conditions.push(build_namespace_name_predicate(&format!(
        "{namespace_relation}.nspname"
    )));
    conditions.push(build_table_predicate(
        &format!("{table_relation}.relkind"),
        &format!("{table_relation}.relname"),
    ));

    Ok((conditions.join(" AND "), params))
}

/// The `PostgreSQL` metadata provider.
#[derive(Clone)]
pub struct PostgreSQLMetadataProvider<'c> {
    connection: &'c Connection,
    platform: &'c PostgreSQLPlatform,
}

impl std::fmt::Debug for PostgreSQLMetadataProvider<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostgreSQLMetadataProvider")
            .field("platform", &self.platform.class())
            .finish_non_exhaustive()
    }
}

impl<'c> PostgreSQLMetadataProvider<'c> {
    /// `@internal` This class can be instantiated only by a database
    /// platform.
    pub fn new(connection: &'c Connection, platform: &'c PostgreSQLPlatform) -> Self {
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

    async fn fetch_first_column(&self, sql: &str) -> Rows<Value> {
        self.connection
            .fetch_first_column(sql, &PhpArray::new(), &ParameterTypes::new())
            .await
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-attrdef.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-attribute.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-type.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-collation.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-depend.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-description.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-inherits.html>
    async fn get_table_columns(
        &self,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<TableColumnMetadataRow> {
        let (predicate, params) = build_table_query_predicate("n", schema_name, "c", table_name)?;

        let sql = format!(
            "SELECT n.nspname,
       c.relname,
       a.attname,
       t.typname,
       format_type(a.atttypid, a.atttypmod),
       bt.typname,
       format_type(bt.oid, t.typtypmod),
       a.attnotnull,
       a.attidentity,
       ({}),
       dsc.description,
       CASE
           WHEN coll.collprovider = 'c'
               THEN coll.collcollate
           WHEN coll.collprovider = 'd'
               THEN NULL
           ELSE coll.collname
           END
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
  AND a.attnum > 0
  AND dep.refobjid IS NULL
  -- exclude partitions (tables that inherit from partitioned tables)
  AND p.oid IS NULL
ORDER BY n.nspname,
         c.relname,
         a.attnum",
            self.platform.get_default_column_value_sql_snippet(),
            predicate,
        );

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| self.create_table_column(row))
            .collect()
    }

    /// `createTableColumn($row)`.
    fn create_table_column(&self, row: &PhpArray) -> Result<TableColumnMetadataRow, Exception> {
        let namespace_name = cell_text(row, 0)?;
        let relation_name = cell_text(row, 1)?;
        let attribute_name = cell_text(row, 2)?;
        let mut type_name = cell_text(row, 3)?;
        let mut complete_type = cell_text(row, 4)?;
        let domain_type_name = cell_nullable(row, 5)?;
        let domain_complete_type = cell_nullable(row, 6)?;
        let is_not_null = cell_bool(row, 7);
        let identity = cell_value(row, 8);
        let default_expression = cell_nullable(row, 9)?;
        let description = cell_nullable(row, 10)?;
        let collation_name = cell_nullable(row, 11)?;

        let mut editor = Column::editor().set_quoted_name(&attribute_name);

        if let Some(domain_type_name) = domain_type_name
            && !self.platform.has_doctrine_type_mapping_for(&type_name)?
        {
            type_name = domain_type_name;
            complete_type = domain_complete_type.unwrap_or_default();
        }

        editor = editor.set_type_name(&self.platform.get_doctrine_type_mapping(&type_name)?);

        match type_name.as_str() {
            "bpchar" | "varchar" => {
                let parameters = parse_column_type_parameters(&complete_type)?;
                if let Some(length) = parameters.first() {
                    editor = editor.set_length(Some(*length));
                }
            }
            "double" | "decimal" | "money" | "numeric" => {
                let parameters = parse_column_type_parameters(&complete_type)?;
                if let Some(precision) = parameters.first() {
                    editor = editor.set_precision(Some(*precision));
                }

                if let Some(scale) = parameters.get(1) {
                    editor = editor.set_scale(*scale);
                }
            }
            _ => {}
        }

        if type_name == "bpchar" {
            editor = editor.set_fixed(true);
        }

        editor = editor
            .set_not_null(is_not_null)
            .set_default_value(parse_default_expression(default_expression.as_deref())?)
            .set_autoincrement(matches!(&identity, Value::String(s) if s.as_bytes() == b"d"));

        if let Some(description) = description {
            editor = editor.set_comment(&description);
        }

        editor = editor.set_collation(collation_name.as_deref());

        Ok(TableColumnMetadataRow::new(
            Some(&namespace_name),
            &relation_name,
            editor.create()?,
        ))
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-index.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    async fn get_index_columns(
        &self,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<IndexColumnMetadataRow> {
        let (predicate, params) = build_table_query_predicate("n", schema_name, "c", table_name)?;

        let sql = format!(
            "SELECT n.nspname,
       c.relname,
       ic.relname,
       i.indisunique,
       pg_get_expr(indpred, indrelid),
       attname
FROM pg_index i
         JOIN pg_class AS c ON c.oid = i.indrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         JOIN pg_class AS ic ON ic.oid = i.indexrelid
         JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS keys(attnum, ord)
              ON TRUE
         JOIN pg_attribute a
              ON a.attrelid = c.oid
                  AND a.attnum = keys.attnum
WHERE {predicate}
  AND i.indisprimary = false
ORDER BY n.nspname,
         c.relname,
         ic.relname,
         keys.ord"
        );

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| {
                Ok(IndexColumnMetadataRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    &cell_text(row, 2)?,
                    if cell_bool(row, 3) {
                        IndexType::Unique
                    } else {
                        IndexType::Regular
                    },
                    false,
                    cell_nullable(row, 4)?.as_deref(),
                    &cell_text(row, 5)?,
                    None,
                ))
            })
            .collect()
    }

    /// The SQL of `getPrimaryKeyConstraintColumns()` (`p`) and
    /// `getUniqueConstraintColumns()` (`u`).
    ///
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-constraint.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-index.html>
    async fn constraint_columns(
        &self,
        contype: &str,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<PhpArray> {
        let (predicate, params) = build_table_query_predicate("n", schema_name, "c", table_name)?;

        let sql = format!(
            "SELECT n.nspname,
       c.relname,
       ct.conname,
       a.attname
FROM pg_namespace n
         INNER JOIN pg_class c
                    ON c.relnamespace = n.oid
         INNER JOIN pg_constraint ct
                    ON ct.conrelid = c.oid
         INNER JOIN pg_index i
                    ON i.indrelid = c.oid
                        AND i.indexrelid = ct.conindid
         INNER JOIN LATERAL unnest(i.indkey) WITH ORDINALITY AS keys(attnum, ord)
                    ON true
         INNER JOIN
     pg_attribute a
     ON a.attrelid = c.oid
         AND a.attnum = keys.attnum
WHERE {predicate}
  AND ct.contype = '{contype}'
ORDER BY n.nspname,
         c.relname,
         ct.conname,
         keys.ord"
        );

        self.fetch_all_numeric(&sql, &params).await
    }

    async fn get_primary_key_constraint_columns(
        &self,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        self.constraint_columns("p", schema_name, table_name)
            .await?
            .iter()
            .map(|row| {
                Ok(PrimaryKeyConstraintColumnRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    Some(&cell_text(row, 2)?),
                    true,
                    &cell_text(row, 3)?,
                ))
            })
            .collect()
    }

    async fn get_unique_constraint_columns(
        &self,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<UniqueConstraintColumnMetadataRow> {
        self.constraint_columns("u", schema_name, table_name)
            .await?
            .iter()
            .map(|row| {
                UniqueConstraintColumnMetadataRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    None,
                    Some(&cell_text(row, 2)?),
                    &cell_text(row, 3)?,
                )
            })
            .collect()
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-constraint.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-attribute.html>
    async fn get_foreign_key_constraint_columns(
        &self,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        let (predicate, params) =
            build_table_query_predicate("pkn", schema_name, "pkc", table_name)?;

        let sql = format!(
            "SELECT pkn.nspname,
       pkc.relname,
       r.conname,
       fkn.nspname,
       fkc.relname,
       r.confupdtype,
       r.confdeltype,
       r.condeferrable,
       r.condeferred,
       pka.attname,
       fka.attname
FROM pg_constraint r
         JOIN pg_class fkc
              ON fkc.oid = r.confrelid
         JOIN pg_namespace fkn
              ON fkn.oid = fkc.relnamespace
         JOIN unnest(r.confkey) WITH ORDINALITY AS fk_attnum(attnum, ord)
              ON TRUE
         JOIN pg_attribute fka
              ON fka.attrelid = fkc.oid
                  AND fka.attnum = fk_attnum.attnum
         JOIN pg_class pkc
              ON pkc.oid = r.conrelid
         JOIN pg_namespace pkn
              ON pkn.oid = pkc.relnamespace
         JOIN unnest(r.conkey) WITH ORDINALITY AS pk_attnum(attnum, ord)
              ON pk_attnum.ord = fk_attnum.ord
         JOIN pg_attribute pka
              ON pka.attrelid = pkc.oid
                  AND pka.attnum = pk_attnum.attnum
WHERE {predicate}
  AND r.contype = 'f'
ORDER BY pkn.nspname,
         pkc.relname,
         r.conname"
        );

        let mut rows = Vec::new();
        for row in self.fetch_all_numeric(&sql, &params).await? {
            let name = cell_text(&row, 2)?;
            rows.push(ForeignKeyConstraintColumnMetadataRow::new(
                ForeignKeyConstraintColumnMetadataRowParts {
                    referencing_schema_name: Some(&cell_text(&row, 0)?),
                    referencing_table_name: &cell_text(&row, 1)?,
                    id: None,
                    name: Some(&name),
                    referenced_schema_name: Some(&cell_text(&row, 3)?),
                    referenced_table_name: &cell_text(&row, 4)?,
                    match_type: MatchType::Simple,
                    on_update_action: referential_action(&cell_text(&row, 5)?)?,
                    on_delete_action: referential_action(&cell_text(&row, 6)?)?,
                    is_deferrable: cell_bool(&row, 7),
                    is_deferred: cell_bool(&row, 8),
                    referencing_column_name: &cell_text(&row, 9)?,
                    referenced_column_name: &cell_text(&row, 10)?,
                },
            )?);
        }

        Ok(rows)
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    async fn get_table_options(
        &self,
        schema_name: Option<&str>,
        table_name: Option<&str>,
    ) -> Rows<TableMetadataRow> {
        let (predicate, params) = build_table_query_predicate("n", schema_name, "c", table_name)?;

        let sql = format!(
            "SELECT n.nspname,
       c.relname,
       CASE c.relpersistence WHEN 'u' THEN true ELSE false END,
       obj_description(c.oid, 'pg_class')
FROM pg_class c
         INNER JOIN pg_namespace n
                    ON n.oid = c.relnamespace
WHERE {predicate}"
        );

        self.fetch_all_numeric(&sql, &params)
            .await?
            .iter()
            .map(|row| {
                let mut options = PhpArray::new();
                options.insert("unlogged", cell_value(row, 2));
                options.insert("comment", cell_value(row, 3));
                Ok(TableMetadataRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    options,
                ))
            })
            .collect()
    }
}

impl MetadataProvider for PostgreSQLMetadataProvider<'_> {
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-database.html>
    async fn get_all_database_names(&self) -> Rows<DatabaseMetadataRow> {
        let sql = "SELECT datname
FROM pg_database
ORDER BY datname";

        self.fetch_first_column(sql)
            .await?
            .iter()
            .map(|database_name| Ok(DatabaseMetadataRow::new(&text_value(Some(database_name))?)))
            .collect()
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    async fn get_all_schema_names(&self) -> Rows<SchemaMetadataRow> {
        let sql = format!(
            "SELECT nspname
FROM pg_namespace
WHERE {}
ORDER BY nspname",
            build_namespace_name_predicate("nspname"),
        );

        self.fetch_first_column(&sql)
            .await?
            .iter()
            .map(|schema_name| Ok(SchemaMetadataRow::new(&text_value(Some(schema_name))?)))
            .collect()
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-class.html>
    /// @link <https://www.postgresql.org/docs/current/catalog-pg-namespace.html>
    async fn get_all_table_names(&self) -> Rows<TableMetadataRow> {
        let sql = format!(
            "SELECT n.nspname,
       c.relname
FROM pg_class c
         INNER JOIN pg_namespace n
                    ON n.oid = c.relnamespace
WHERE {}
  AND {}
ORDER BY n.nspname,
         c.relname",
            build_namespace_name_predicate("n.nspname"),
            build_table_predicate("c.relkind", "c.relname"),
        );

        self.fetch_all_numeric(&sql, &[])
            .await?
            .iter()
            .map(|row| {
                Ok(TableMetadataRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    PhpArray::new(),
                ))
            })
            .collect()
    }

    async fn get_table_columns_for_all_tables(&self) -> Rows<TableColumnMetadataRow> {
        self.get_table_columns(None, None).await
    }

    async fn get_table_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableColumnMetadataRow> {
        let Some(schema_name) = schema_name else {
            return Err(unsupported_name_from_null_schema_name(&method(
                "getTableColumnsForTable",
            )));
        };
        self.get_table_columns(Some(schema_name), Some(table_name))
            .await
    }

    async fn get_index_columns_for_all_tables(&self) -> Rows<IndexColumnMetadataRow> {
        self.get_index_columns(None, None).await
    }

    async fn get_index_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<IndexColumnMetadataRow> {
        let Some(schema_name) = schema_name else {
            return Err(unsupported_name_from_null_schema_name(&method(
                "getIndexColumnsForTable",
            )));
        };
        self.get_index_columns(Some(schema_name), Some(table_name))
            .await
    }

    async fn get_primary_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        self.get_primary_key_constraint_columns(None, None).await
    }

    async fn get_primary_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<PrimaryKeyConstraintColumnRow> {
        let Some(schema_name) = schema_name else {
            return Err(unsupported_name_from_null_schema_name(&method(
                "getPrimaryKeyConstraintColumnsForTable",
            )));
        };
        self.get_primary_key_constraint_columns(Some(schema_name), Some(table_name))
            .await
    }

    async fn get_unique_constraint_columns_for_all_tables(
        &self,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        Some(self.get_unique_constraint_columns(None, None).await)
    }

    async fn get_unique_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Option<Rows<UniqueConstraintColumnMetadataRow>> {
        let Some(schema_name) = schema_name else {
            return Some(Err(unsupported_name_from_null_schema_name(&method(
                "getUniqueConstraintColumnsForTable",
            ))));
        };
        Some(
            self.get_unique_constraint_columns(Some(schema_name), Some(table_name))
                .await,
        )
    }

    async fn get_foreign_key_constraint_columns_for_all_tables(
        &self,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        self.get_foreign_key_constraint_columns(None, None).await
    }

    async fn get_foreign_key_constraint_columns_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<ForeignKeyConstraintColumnMetadataRow> {
        let Some(schema_name) = schema_name else {
            return Err(unsupported_name_from_null_schema_name(&method(
                "getForeignKeyConstraintColumnsForTable",
            )));
        };
        self.get_foreign_key_constraint_columns(Some(schema_name), Some(table_name))
            .await
    }

    async fn get_table_options_for_all_tables(&self) -> Rows<TableMetadataRow> {
        self.get_table_options(None, None).await
    }

    async fn get_table_options_for_table(
        &self,
        schema_name: Option<&str>,
        table_name: &str,
    ) -> Rows<TableMetadataRow> {
        let Some(schema_name) = schema_name else {
            return Err(unsupported_name_from_null_schema_name(&method(
                "getTableOptionsForTable",
            )));
        };
        self.get_table_options(Some(schema_name), Some(table_name))
            .await
    }

    /// @link <https://www.postgresql.org/docs/current/catalog-pg-views.html>
    async fn get_all_views(&self) -> Rows<ViewMetadataRow> {
        let sql = format!(
            "SELECT schemaname,
       viewname,
       definition
FROM pg_views
WHERE {}
ORDER BY schemaname,
         viewname",
            build_namespace_name_predicate("schemaname"),
        );

        self.fetch_all_numeric(&sql, &[])
            .await?
            .iter()
            .map(|row| {
                Ok(ViewMetadataRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    &cell_text(row, 2)?,
                ))
            })
            .collect()
    }

    async fn get_all_sequences(&self) -> Rows<SequenceMetadataRow> {
        // Using information_schema here instead of pg_sequences since the latter also includes sequences that are owned
        // by serial columns. We want to include only the user-defined ones.
        let sql = format!(
            "SELECT sequence_schema,
       sequence_name,
       increment,
       minimum_value
FROM information_schema.sequences
WHERE sequence_catalog = CURRENT_DATABASE()
  AND {}
ORDER BY sequence_schema,
         sequence_name",
            build_namespace_name_predicate("sequence_schema"),
        );

        self.fetch_all_numeric(&sql, &[])
            .await?
            .iter()
            .map(|row| {
                Ok(SequenceMetadataRow::new(
                    Some(&cell_text(row, 0)?),
                    &cell_text(row, 1)?,
                    cell_int(row, 2),
                    cell_int(row, 3),
                    None,
                ))
            })
            .collect()
    }

    fn class(&self) -> &str {
        CLASS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_column_type_parameters() {
        assert_eq!(
            parse_column_type_parameters("character varying(255)").ok(),
            Some(vec![255])
        );
        assert_eq!(
            parse_column_type_parameters("numeric(10,2)").ok(),
            Some(vec![10, 2])
        );
        assert_eq!(parse_column_type_parameters("text").ok(), Some(vec![]));
    }

    #[test]
    fn parses_default_expressions() {
        assert_eq!(parse_default_expression(None).ok(), Some(Value::Null));
        assert_eq!(
            parse_default_expression(Some("NULL::character varying")).ok(),
            Some(Value::Null)
        );
        assert_eq!(
            parse_default_expression(Some("true")).ok(),
            Some(Value::Bool(true))
        );
        assert_eq!(
            parse_default_expression(Some("'it''s'::text")).ok(),
            Some(Value::string("it's"))
        );
        assert_eq!(
            parse_default_expression(Some("now()")).ok(),
            Some(Value::string("now()"))
        );
    }

    #[test]
    fn builds_the_table_query_predicate() {
        let (sql, params) =
            build_table_query_predicate("n", Some("public"), "c", Some("t")).unwrap_or_default();
        assert_eq!(
            sql,
            "n.nspname = ? AND c.relname = ? AND n.nspname NOT LIKE 'pg\\_%' AND n.nspname != 'information_schema' AND c.relkind IN ('r', 'p') AND c.relname NOT IN ('geometry_columns', 'spatial_ref_sys')"
        );
        assert_eq!(params, vec!["public".to_owned(), "t".to_owned()]);
        assert!(build_table_query_predicate("n", None, "c", Some("t")).is_err());
    }
}
