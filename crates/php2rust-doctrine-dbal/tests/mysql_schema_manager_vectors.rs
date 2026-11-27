//! `MySQLSchemaManager`, `MySQLMetadataProvider` and the `MySQL`
//! comparator's connection-backed metadata providers against what PHP 8.4
//! gives through doctrine/dbal (`tests/data/mysql_schema.json`,
//! `scripts/record.sh dbal-mysql-schema`). No server: a scripted driver
//! answers each query with the first recorded response whose needle the
//! SQL contains, as `information_schema` would. Each case checks the
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
    AbstractMySQLPlatform, MariaDB1010Platform, MariaDB120300Platform, MariaDBPlatform,
    MySQL80Platform, MySQLPlatform,
};
use php2rust_doctrine_dbal::schema::{AbstractSchemaManager, Column, MySQLSchemaManager, Table};
use php2rust_doctrine_dbal::types::Types;
use php2rust_doctrine_dbal::{Connection, Exception, ParameterType, Query, ServerVersionProvider};

fn data() -> &'static serde_json::Value {
    static DATA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/mysql_schema.json"
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

fn platform_box(name: &str) -> PlatformBox {
    match name {
        "MySQLPlatform" => Box::new(MySQLPlatform::new()),
        "MySQL80Platform" => Box::new(MySQL80Platform::new()),
        "MariaDBPlatform" => Box::new(MariaDBPlatform::new()),
        "MariaDB1010Platform" => Box::new(MariaDB1010Platform::new()),
        "MariaDB120300Platform" => Box::new(MariaDB120300Platform::new()),
        other => panic!("platform {other}"),
    }
}

fn mysql_platform(name: &str) -> Box<dyn AbstractMySQLPlatform> {
    match name {
        "MySQLPlatform" => Box::new(MySQLPlatform::new()),
        "MySQL80Platform" => Box::new(MySQL80Platform::new()),
        "MariaDBPlatform" => Box::new(MariaDBPlatform::new()),
        "MariaDB1010Platform" => Box::new(MariaDB1010Platform::new()),
        "MariaDB120300Platform" => Box::new(MariaDB120300Platform::new()),
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
        Box::pin(async move { Ok(platform_box(&self.platform)) })
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
        Box::pin(async move { Ok("8.4.0".to_owned()) })
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
            string("a").set_charset(Some("utf8mb4")).create().unwrap(),
            string("b")
                .set_collation(Some("latin1_swedish_ci"))
                .create()
                .unwrap(),
            string("c").create().unwrap(),
        ])
        .create()
        .unwrap();
    let new = Table::editor()
        .set_unquoted_name("t", None)
        .set_columns(vec![
            string("a")
                .set_collation(Some("utf8mb4_general_ci"))
                .create()
                .unwrap(),
            string("b").set_charset(Some("latin1")).create().unwrap(),
            string("c")
                .set_collation(Some("utf8mb4_bin"))
                .create()
                .unwrap(),
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

async fn run_case(
    call: &str,
    manager: &MySQLSchemaManager<'_>,
    platform: &dyn AbstractMySQLPlatform,
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
        "listTableColumns" => {
            let mut columns = PhpArray::new();
            for (key, column) in manager.list_table_columns("t").await?.iter() {
                columns.insert(Key::from(key), column_array(column)?);
            }
            Ok(Value::Array(columns))
        }
        "compare" => {
            let (old, new) = compare_tables();
            let diff = manager
                .create_comparator()
                .await?
                .compare_tables(&old, &new)?;
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
        let platform = mysql_platform(platform_name);
        let manager = MySQLSchemaManager::new(&connection, platform.as_ref());

        let outcome = run_case(case["call"].as_str().unwrap(), &manager, platform.as_ref()).await;

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
