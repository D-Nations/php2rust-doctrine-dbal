//! `PostgreSQLSchemaManager`, `PostgreSQLMetadataProvider` and the
//! `PostgreSQL` comparator against what PHP 8.4 gives through
//! doctrine/dbal (`tests/data/postgresql_schema.json`,
//! `scripts/record.sh dbal-pgsql-schema`). No server: a scripted driver
//! answers each query with the first recorded response whose needle the
//! SQL contains, as the system catalogs would. Each case checks the
//! queries the call ran (SQL and parameters) and its outcome.
//!
//! Upstream: none
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::vectors::decode;
use php_value::{Key, PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::cache::ArrayResult;
use php2rust_doctrine_dbal::driver::api::ExceptionConverter;
use php2rust_doctrine_dbal::driver::{
    BoxFuture, Connection as DriverConnection, Driver, PlatformBox, Result as DriverResult,
    Statement as DriverStatement,
};
use php2rust_doctrine_dbal::platforms::{
    AbstractPlatform, PostgreSQL120Platform, PostgreSQLPlatform,
};
use php2rust_doctrine_dbal::schema::{
    AbstractSchemaManager, Column, ForeignKeyConstraint, Index, PostgreSQLSchemaManager, Table,
};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, Exception, ParameterType, Query, ServerVersionProvider};

fn data() -> &'static serde_json::Value {
    static DATA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/postgresql_schema.json"
        ))
        .unwrap();
        serde_json::from_str(&text).unwrap()
    })
}

/// A recorded response: the needle, the column names and the rows.
struct Response {
    needle: String,
    columns: Vec<PhpString>,
    rows: Vec<PhpArray>,
}

/// The responses and the queries the connection got.
struct Script {
    responses: Vec<Response>,
    queries: Mutex<Vec<(String, Vec<Value>)>>,
}

impl Script {
    fn answer(&self, sql: &str, params: Vec<Value>) -> Box<dyn DriverResult> {
        self.queries.lock().unwrap().push((sql.to_owned(), params));
        for response in &self.responses {
            if sql.contains(&response.needle) {
                return Box::new(ArrayResult::new(
                    response.columns.clone(),
                    response.rows.clone(),
                ));
            }
        }
        Box::new(ArrayResult::new(vec![PhpString::from("none")], Vec::new()))
    }
}

struct ScriptDriver {
    script: Arc<Script>,
    platform: String,
}

struct ScriptConnection {
    script: Arc<Script>,
}

struct ScriptStatement {
    script: Arc<Script>,
    sql: String,
    params: Vec<(i64, Value)>,
}

struct ScriptConverter;

impl ExceptionConverter for ScriptConverter {
    fn convert(&self, exception: Exception, query: Option<Query>) -> Exception {
        Exception::driver_exception(Exception::DRIVER_EXCEPTION, exception, query)
    }
}

fn postgresql_platform(name: &str) -> PostgreSQLPlatform {
    match name {
        "PostgreSQLPlatform" => PostgreSQLPlatform::new(),
        "PostgreSQL120Platform" => PostgreSQL120Platform::new(),
        other => panic!("platform {other}"),
    }
}

impl Driver for ScriptDriver {
    fn class(&self) -> &'static str {
        "ScriptDriver"
    }

    fn connect<'a>(
        &'a self,
        _params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn DriverConnection>, Exception>> {
        Box::pin(async move {
            let connection: Arc<dyn DriverConnection> = Arc::new(ScriptConnection {
                script: self.script.clone(),
            });
            Ok(connection)
        })
    }

    fn get_database_platform<'a>(
        &'a self,
        _version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async move {
            let platform: PlatformBox = Box::new(postgresql_platform(&self.platform));
            Ok(platform)
        })
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(ScriptConverter)
    }
}

impl DriverConnection for ScriptConnection {
    fn prepare<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverStatement>, Exception>> {
        Box::pin(async move {
            let statement: Box<dyn DriverStatement> = Box::new(ScriptStatement {
                script: self.script.clone(),
                sql: sql.to_owned(),
                params: Vec::new(),
            });
            Ok(statement)
        })
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move { Ok(self.script.answer(sql, Vec::new())) })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        let mut quoted = b"'".to_vec();
        for &b in value {
            if b == b'\'' {
                quoted.push(b'\'');
            }
            quoted.push(b);
        }
        quoted.push(b'\'');
        PhpString::from(quoted)
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async move {
            self.script
                .queries
                .lock()
                .unwrap()
                .push((sql.to_owned(), Vec::new()));
            Ok(0)
        })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async move { Ok(Value::Int(0)) })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move { Ok(()) })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move { Ok(()) })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move { Ok(()) })
    }

    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        Box::pin(async move { Ok("17.0".to_owned()) })
    }

    fn get_native_connection(&self) -> php2rust_doctrine_dbal::driver::NativeConnection {
        Arc::new(())
    }
}

impl DriverStatement for ScriptStatement {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        _type: ParameterType,
    ) -> Result<(), Exception> {
        let Key::Int(param) = param else {
            panic!("named parameter {param:?}");
        };
        self.params.retain(|(p, _)| p != param);
        self.params.push((*param, value));
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            self.params.sort_by_key(|(p, _)| *p);
            let params = self.params.iter().map(|(_, v)| v.clone()).collect();
            Ok(self.script.answer(&self.sql, params))
        })
    }
}

fn list(value: Value) -> Vec<Value> {
    match value {
        Value::Array(array) => array.values().cloned().collect(),
        other => panic!("not an array: {other:?}"),
    }
}

fn responses(json: &serde_json::Value) -> Vec<Response> {
    json.as_array()
        .unwrap()
        .iter()
        .map(|response| Response {
            needle: response[0].as_str().unwrap().to_owned(),
            columns: response[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| PhpString::from(c.as_str().unwrap()))
                .collect(),
            rows: list(decode(&response[2]).unwrap())
                .into_iter()
                .map(|row| match row {
                    Value::Array(row) => row,
                    other => panic!("row {other:?}"),
                })
                .collect(),
        })
        .collect()
}

fn strings(sql: Vec<String>) -> Value {
    Value::Array(PhpArray::list(sql.into_iter().map(Value::string)))
}

/// `compare_tables()` of the recorder.
fn compare_tables() -> (Table, Table) {
    let string = |name: &str| {
        Column::editor()
            .set_unquoted_name(name)
            .set_type_name(Types::STRING)
            .set_length(Some(10))
    };
    let old = Table::editor()
        .set_unquoted_name("t", None)
        .set_columns(vec![
            string("a").create().unwrap(),
            string("b").create().unwrap(),
        ])
        .create()
        .unwrap();
    let new = Table::editor()
        .set_unquoted_name("t", None)
        .set_columns(vec![
            string("a").set_collation(Some("default")).create().unwrap(),
            string("b").set_collation(Some("C")).create().unwrap(),
        ])
        .create()
        .unwrap();
    (old, new)
}

/// `$column->toArray()` with the type's class and a default expression
/// as `["expression", class]`.
fn column_array(column: &Column) -> Result<Value, Exception> {
    let mut array = column.to_array(false)?;
    array.insert("type", Value::string(column.get_type()?.class()));
    if let Some(Value::Object(object)) = array.get(&Key::from("default")) {
        let expression =
            PhpArray::list([Value::string("expression"), Value::string(object.class())]);
        array.insert("default", Value::Array(expression));
    }
    Ok(Value::Array(array))
}

fn strs(values: Vec<String>) -> Value {
    strings(values)
}

fn index_array(index: &Index) -> Value {
    Value::Array(PhpArray::list([
        Value::string(index.get_name()),
        strs(index.get_columns()),
        Value::Bool(index.is_unique()),
        Value::Bool(index.is_primary()),
        strs(index.get_flags()),
        Value::Array(index.get_options().clone()),
    ]))
}

fn foreign_key_array(fk: &ForeignKeyConstraint) -> Value {
    Value::Array(PhpArray::list([
        Value::string(fk.get_name()),
        strs(fk.get_local_columns()),
        Value::string(fk.get_foreign_table_name()),
        strs(fk.get_foreign_columns()),
        Value::Array(fk.get_options().clone()),
    ]))
}

fn pairs<T>(items: &[T], f: impl Fn(&T) -> Vec<Value>) -> Value {
    Value::Array(PhpArray::list(
        items
            .iter()
            .map(|item| Value::Array(PhpArray::list(f(item)))),
    ))
}

async fn run_case(
    call: &str,
    manager: &PostgreSQLSchemaManager<'_>,
    platform: &PostgreSQLPlatform,
) -> Result<Value, Exception> {
    match call {
        "introspectTable" => Ok(strings(platform.get_create_table_sql(
            &manager.introspect_table_by_unquoted_name("t", None).await?,
        )?)),
        "listTables" => {
            let mut tables = Vec::new();
            for table in manager.list_tables().await? {
                tables.push(strings(platform.get_create_table_sql(&table)?));
            }
            Ok(Value::Array(PhpArray::list(tables)))
        }
        "listTableColumns" | "listTableColumnsQualified" => {
            let table = if call == "listTableColumns" {
                "t"
            } else {
                "other_schema.u"
            };
            let mut columns = PhpArray::new();
            for (key, column) in manager.list_table_columns(table).await?.iter() {
                columns.insert(Key::from(key), column_array(column)?);
            }
            Ok(Value::Array(columns))
        }
        "listTableIndexes" => {
            let mut indexes = PhpArray::new();
            for (key, index) in manager.list_table_indexes("t").await?.iter() {
                indexes.insert(Key::from(key), index_array(index));
            }
            Ok(Value::Array(indexes))
        }
        "listTableForeignKeys" => Ok(Value::Array(PhpArray::list(
            manager
                .list_table_foreign_keys("t")
                .await?
                .iter()
                .map(foreign_key_array),
        ))),
        "listTableNames" => Ok(strings(manager.list_table_names().await?)),
        "listSchemaNames" => Ok(strings(manager.list_schema_names().await?)),
        "listDatabases" => Ok(strings(manager.list_databases().await?)),
        "listViews" | "introspectViews" => {
            let views = if call == "listViews" {
                manager.list_views().await?
            } else {
                manager.introspect_views().await?
            };
            Ok(pairs(&views, |v| {
                vec![Value::string(v.get_name()), Value::string(v.get_sql())]
            }))
        }
        "listSequences" | "introspectSequences" => {
            let sequences = if call == "listSequences" {
                manager.list_sequences().await?
            } else {
                manager.introspect_sequences().await?
            };
            Ok(pairs(&sequences, |s| {
                vec![
                    Value::string(s.get_name()),
                    Value::Int(s.get_allocation_size()),
                    Value::Int(s.get_initial_value()),
                ]
            }))
        }
        "introspectDatabaseNames" => Ok(strings(
            manager
                .introspect_database_names()
                .await?
                .iter()
                .map(ToString::to_string)
                .collect(),
        )),
        "introspectSchemaNames" => Ok(strings(
            manager
                .introspect_schema_names()
                .await?
                .iter()
                .map(ToString::to_string)
                .collect(),
        )),
        "introspectTableNames" => Ok(strings(
            manager
                .introspect_table_names()
                .await?
                .iter()
                .map(ToString::to_string)
                .collect(),
        )),
        "compare" => {
            let (old, new) = compare_tables();
            let diff = manager.create_comparator().compare_tables(&old, &new)?;
            Ok(strings(platform.get_alter_table_sql(&diff)?))
        }
        other => panic!("call {other}"),
    }
}

#[tokio::test]
async fn schemaManagerCases() {
    for case in data()["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let platform_name = case["platform"].as_str().unwrap();
        let script = Arc::new(Script {
            responses: responses(&case["responses"]),
            queries: Mutex::new(Vec::new()),
        });
        let connection = Connection::new(
            PhpArray::new(),
            Arc::new(ScriptDriver {
                script: script.clone(),
                platform: platform_name.to_owned(),
            }),
            None,
        );
        let platform = postgresql_platform(platform_name);
        let manager = PostgreSQLSchemaManager::new(&connection, &platform);

        let outcome = run_case(case["call"].as_str().unwrap(), &manager, &platform).await;

        let expected = &case["outcome"];
        match (expected[0].as_str(), outcome) {
            (Some("ok"), Ok(value)) => {
                let expected = decode(&expected[1]).unwrap();
                assert!(
                    expected.identical(&value),
                    "{name}: expected {expected:#?}, got {value:#?}"
                );
            }
            (Some("throws"), Err(e)) => {
                assert_eq!(e.class(), expected[1].as_str().unwrap(), "{name}: {e}");
                assert_eq!(
                    e.message().to_string_lossy(),
                    expected[2].as_str().unwrap(),
                    "{name}"
                );
            }
            (_, outcome) => panic!("{name}: expected {expected}, got {outcome:?}"),
        }

        let queries = script.queries.lock().unwrap().clone();
        let expected: Vec<(String, Vec<Value>)> = case["queries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|q| {
                (
                    q[0].as_str().unwrap().to_owned(),
                    list(decode(&q[1]).unwrap()),
                )
            })
            .collect();
        assert_eq!(queries.len(), expected.len(), "{name}: {queries:#?}");
        for ((sql, params), (expected_sql, expected_params)) in queries.iter().zip(&expected) {
            assert_eq!(sql, expected_sql, "{name}");
            assert_eq!(params.len(), expected_params.len(), "{name}: {sql}");
            for (param, expected_param) in params.iter().zip(expected_params) {
                assert!(param.identical(expected_param), "{name}: {sql}");
            }
        }
    }
}
