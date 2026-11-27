//! The `PostgreSQL` driver (the `pdo_pgsql` cargo feature). It stands where
//! Doctrine's `Driver\PDO\PgSQL\Driver` stands (with its `PDO\Connection`,
//! `PDO\Statement`, `PDO\Result` and `PDO\Exception`), and gives what
//! `pdo_pgsql` gives in PHP 8.4.27 (`ext/pdo_pgsql`), on the libpq calls
//! of [`libpq`]:
//!
//! - DBAL turns on `Pdo\Pgsql::ATTR_DISABLE_PREPARES`, so a statement runs
//!   with `PQexecParams()` on the unnamed statement: its `?` and `:name`
//!   placeholders become `$1`, `$2`... (a name used twice is one
//!   parameter), each parameter is sent as text with an unknown type, a
//!   `BINARY` or `LARGE_OBJECT` one as bytes. `query()` is a statement
//!   without parameters (one SQL statement); `exec()` is `PQexec()` (several
//!   statements allowed).
//! - A bound value is converted as PDO does for its type: `null` and
//!   `NULL` bind SQL NULL; a bool binds `t` or `f` (`BOOLEAN` makes an int
//!   a bool, `INTEGER` makes a bool an int); everything else binds
//!   `(string) $value`. A parameter the statement does not have fails at
//!   bind time with `SQLSTATE[HY093]`.
//! - Values come back as `pdo_pgsql` converts the text of their type:
//!   `bool` as a bool, `int2`, `int4`, `int8` and `oid` as an int, `float4`
//!   and `float8` as a float (`Infinity`, `-Infinity`, `NaN` included),
//!   `bytea` as its bytes (PHP gives a stream holding them), the others as
//!   strings, SQL NULL as null.
//! - Errors read `SQLSTATE[<state>]: <description>: 7 <libpq message>`
//!   (`7` is `PGRES_FATAL_ERROR`); connecting fails with `SQLSTATE[08006]
//!   [7] <libpq message>`.
//! - `rowCount()` is the number of rows of a result with rows, else the
//!   count of the command tag; `lastInsertId()` is `SELECT LASTVAL()`
//!   (`NoIdentityValue` on `SQLSTATE` 55000).
//! - `quote()` is `PQescapeStringConn()` in single quotes.
//! - The transaction state is the server's (`PQtransactionStatus()`).
//!
//! What PDO does that this driver does not (README.md, Differences):
//! `persistent` connections and the other `driverOptions` (PDO attributes)
//! are ignored; prepares stay disabled whatever
//! `ATTR_DISABLE_PREPARES` says; `??` is not turned into `?`; TLS is not
//! supported.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/PDO/PgSQL/Driver.php

mod libpq;
mod sqlstate;

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use php_value::{Key, PhpArray, PhpString, Value};
use tokio::sync::Mutex;

use self::libpq::{ConnInfo, Param, PgConn, PgError, PgResult};
use super::api::ExceptionConverter;
use super::{
    AbstractPostgreSQLDriver, BoxFuture, Connection as DriverConnection, Driver as DriverInterface,
    NativeConnection, PlatformBox, Result as DriverResult, Statement as DriverStatement,
};
use crate::sql::{Parser, Visitor};
use crate::{Exception, ParameterType, ServerVersionProvider};

/// `Doctrine\DBAL\Driver\PDO\PgSQL\Driver`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Driver;

impl Driver {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Driver\\PDO\\PgSQL\\Driver";

    pub fn new() -> Self {
        Self
    }
}

impl AbstractPostgreSQLDriver for Driver {}

/// `InvalidConfiguration::notAStringOrNull($key, $value)`.
fn not_a_string_or_null(key: &str, value: &Value) -> Exception {
    Exception::driver(
        Exception::PDO_INVALID_CONFIGURATION,
        format!(
            "The {key} configuration parameter is expected to be either a string or null, got {}.",
            value.debug_type()
        ),
        None,
        0,
    )
}

/// `isset($params[$key])`.
fn isset<'a>(params: &'a PhpArray, key: &str) -> Option<&'a Value> {
    params.get(&Key::from(key)).filter(|value| !value.is_null())
}

/// `(string) $params[$key]` when it is set (and not `''` for the keys
/// that `constructPdoDsn()` skips when empty).
fn dsn_value(params: &PhpArray, key: &str) -> Option<String> {
    isset(params, key)
        .and_then(Value::to_php_string)
        .map(|s| s.to_string_lossy().into_owned())
}

/// `_pdo_pgsql_trim_message()`: libpq's message without its trailing
/// newlines.
fn trim_message(message: &str) -> &str {
    let bytes = message.as_bytes();
    if bytes.is_empty() {
        return message;
    }
    let mut i = bytes.len() - 1;
    if i > 1 && matches!(bytes[i - 1], b'\r' | b'\n') && bytes[i] == b'.' {
        i -= 1;
    }
    while i > 0 && matches!(bytes[i], b'\r' | b'\n') {
        i -= 1;
    }
    &message[..=i]
}

/// `PDO\Exception::new($pdoException)` for a libpq error of a statement:
/// `SQLSTATE[...]: <description>: <status> <message>`.
fn pdo_exception(error: &PgError) -> Exception {
    let state = error
        .sql_state
        .clone()
        .filter(|state| state.len() == 5)
        .unwrap_or_else(|| "HY000".to_owned());
    let description = sqlstate::description(&state);
    let message = trim_message(&error.message);
    let text = if error.status != 0 {
        format!(
            "SQLSTATE[{state}]: {description}: {} {message}",
            error.status
        )
    } else {
        format!("SQLSTATE[{state}]: {description}: {message}")
    };
    Exception::driver(Exception::PDO_EXCEPTION, text, Some(state), error.status)
}

/// `pdo_raise_impl_error()`: an error PDO raises itself (no native code).
fn pdo_impl_error(state: &str, supplement: &str) -> Exception {
    Exception::driver(
        Exception::PDO_EXCEPTION,
        format!(
            "SQLSTATE[{state}]: {}: {supplement}",
            sqlstate::description(state)
        ),
        Some(state.to_owned()),
        0,
    )
}

/// A PDO error with a plain message (no `SQLSTATE`).
fn pdo_message(message: &str) -> Exception {
    Exception::driver(Exception::PDO_EXCEPTION, message, None, 0)
}

/// PDO's `ValueError` for an empty statement.
fn empty_statement(method: &str, parameter: &str) -> Exception {
    Exception::new(
        Exception::VALUE_ERROR,
        format!("PDO::{method}(): Argument #1 (${parameter}) must not be empty"),
    )
}

impl DriverInterface for Driver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn connect<'a>(
        &'a self,
        params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn DriverConnection>, Exception>> {
        Box::pin(async move {
            for key in ["user", "password"] {
                if let Some(value) = isset(params, key)
                    && !matches!(value, Value::String(_))
                {
                    return Err(not_a_string_or_null(key, value));
                }
            }

            let info = ConnInfo {
                host: dsn_value(params, "host").filter(|host| !host.is_empty()),
                port: dsn_value(params, "port").filter(|port| !port.is_empty()),
                dbname: dsn_value(params, "dbname"),
                user: dsn_value(params, "user").unwrap_or_default(),
                password: dsn_value(params, "password").unwrap_or_default(),
                sslmode: dsn_value(params, "sslmode"),
                application_name: dsn_value(params, "application_name"),
                connect_timeout: 30,
            };
            let connection = PgConn::connect(&info).await.map_err(|error| {
                let state = "08006";
                Exception::driver(
                    Exception::PDO_EXCEPTION,
                    format!(
                        "SQLSTATE[{state}] [{}] {}",
                        error.status,
                        trim_message(&error.message)
                    ),
                    Some(state.to_owned()),
                    error.status,
                )
            })?;

            let connection = PdoConnection {
                inner: Arc::new(Mutex::new(connection)),
            };

            // Defining client_encoding via SET NAMES to avoid inconsistent
            // DSN support (passing client_encoding via the 'options'
            // param breaks pgbouncer support).
            if let Some(charset) = dsn_value(params, "charset") {
                connection.exec(&format!("SET NAMES '{charset}'")).await?;
            }

            Ok(Arc::new(connection) as Arc<dyn DriverConnection>)
        })
    }

    fn get_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        self.postgresql_database_platform(version_provider)
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        self.postgresql_exception_converter()
    }
}

/// `Doctrine\DBAL\Driver\PDO\Connection` on a `PostgreSQL` server.
struct PdoConnection {
    inner: Arc<Mutex<PgConn>>,
}

/// The SQL that `PQexecParams()` runs: placeholders as `$n`, `names` the
/// `$n` of each `:name`.
#[derive(Default)]
struct Placeholders {
    sql: Vec<u8>,
    names: Vec<(Vec<u8>, usize)>,
    count: usize,
}

impl Visitor for Placeholders {
    type Error = std::convert::Infallible;

    fn accept_positional_parameter(&mut self, _sql: &[u8]) -> Result<(), Self::Error> {
        self.count += 1;
        self.sql
            .extend_from_slice(format!("${}", self.count).as_bytes());
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        let index = if let Some((_, index)) = self.names.iter().find(|(name, _)| name == sql) {
            *index
        } else {
            self.count += 1;
            self.names.push((sql.to_vec(), self.count));
            self.count
        };
        self.sql.extend_from_slice(format!("${index}").as_bytes());
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.sql.extend_from_slice(sql);
        Ok(())
    }
}

/// A prepared statement: the SQL with `$n` placeholders, its named
/// parameters and its parameter count.
struct Prepared {
    sql: String,
    names: Vec<(Vec<u8>, usize)>,
    parameter_count: usize,
}

fn prepare(sql: &str) -> Result<Prepared, Exception> {
    let mut placeholders = Placeholders::default();
    let Ok(()) = Parser::new(false).parse(sql.as_bytes(), &mut placeholders);
    if placeholders.count == 0 {
        return Ok(Prepared {
            sql: sql.to_owned(),
            names: Vec::new(),
            parameter_count: 0,
        });
    }
    // The rewrite only replaces ASCII placeholders in the text.
    let rewritten = String::from_utf8(placeholders.sql)
        .map_err(|_| Exception::new(Exception::UNSUPPORTED, "SQL that is not UTF-8"))?;
    Ok(Prepared {
        sql: rewritten,
        names: placeholders.names,
        parameter_count: placeholders.count,
    })
}

/// `ZEND_ATOL()`: the leading integer (saturating, as `strtol()`).
fn atol(text: &[u8]) -> i64 {
    let mut rest = text;
    while let Some((first, tail)) = rest.split_first() {
        if first.is_ascii_whitespace() {
            rest = tail;
        } else {
            break;
        }
    }
    let negative = rest.first() == Some(&b'-');
    if matches!(rest.first(), Some(b'-' | b'+')) {
        rest = &rest[1..];
    }
    let mut value: i64 = 0;
    for &b in rest.iter().take_while(|b| b.is_ascii_digit()) {
        let digit = i64::from(b - b'0');
        value = if negative {
            value.saturating_mul(10).saturating_sub(digit)
        } else {
            value.saturating_mul(10).saturating_add(digit)
        };
    }
    value
}

/// The type OIDs `pdo_pgsql` converts.
const BOOLOID: u32 = 16;
const BYTEAOID: u32 = 17;
const INT8OID: u32 = 20;
const INT2OID: u32 = 21;
const INT4OID: u32 = 23;
const OIDOID: u32 = 26;
const FLOAT4OID: u32 = 700;
const FLOAT8OID: u32 = 701;

/// `pgsql_stmt_get_col()`: a value as `pdo_pgsql` gives it.
fn column_value(type_oid: u32, text: Option<&[u8]>) -> Value {
    let Some(text) = text else {
        return Value::Null;
    };
    match type_oid {
        BOOLOID => Value::Bool(text.first() == Some(&b't')),
        INT2OID | INT4OID | INT8OID => Value::Int(atol(text)),
        FLOAT4OID | FLOAT8OID => Value::Float(match text {
            b"Infinity" => f64::INFINITY,
            b"-Infinity" => f64::NEG_INFINITY,
            b"NaN" => f64::NAN,
            _ => php_value::numeric::string_to_float(text),
        }),
        OIDOID => {
            let digits = text.iter().take_while(|b| b.is_ascii_digit()).count();
            Value::Int(
                std::str::from_utf8(&text[..digits])
                    .ok()
                    .and_then(|d| d.parse::<u32>().ok())
                    .map_or(0, i64::from),
            )
        }
        BYTEAOID => Value::String(PhpString::from(libpq::unescape_bytea(text))),
        _ => Value::String(PhpString::from(text.to_vec())),
    }
}

/// The result of a statement, read into memory.
fn result(result: PgResult) -> PdoResult {
    let row_count = if result.tuples {
        i64::try_from(result.rows.len()).unwrap_or(i64::MAX)
    } else {
        atol(result.cmd_tuples().as_bytes())
    };
    let types: Vec<u32> = result.fields.iter().map(|f| f.type_oid).collect();
    let rows = result
        .rows
        .into_iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(index, value)| {
                    column_value(types.get(index).copied().unwrap_or(0), value.as_deref())
                })
                .collect()
        })
        .collect();
    PdoResult {
        columns: result
            .fields
            .into_iter()
            .map(|field| PhpString::from(field.name))
            .collect(),
        rows,
        row_count,
    }
}

/// `pdo_pgsql_transaction_cmd()`.
async fn transaction_statement(connection: &mut PgConn, sql: &str) -> Result<(), Exception> {
    connection
        .exec(sql)
        .await
        .map(|_| ())
        .map_err(|e| pdo_exception(&e))
}

impl DriverConnection for PdoConnection {
    fn prepare<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverStatement>, Exception>> {
        Box::pin(async move {
            if sql.is_empty() {
                return Err(empty_statement("prepare", "query"));
            }
            Ok(Box::new(PdoStatement {
                inner: Arc::clone(&self.inner),
                prepared: prepare(sql)?,
                bound: BTreeMap::new(),
            }) as Box<dyn DriverStatement>)
        })
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            if sql.is_empty() {
                return Err(empty_statement("query", "query"));
            }
            let prepared = prepare(sql)?;
            let mut connection = self.inner.lock().await;
            let outcome = connection.exec_params(&prepared.sql, &[]).await;
            outcome
                .map(|r| Box::new(result(r)) as Box<dyn DriverResult>)
                .map_err(|e| pdo_exception(&e))
        })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        if let Ok(connection) = self.inner.try_lock() {
            return PhpString::from(connection.quote(value));
        }
        // While a query runs, quote as with standard_conforming_strings on.
        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
        let mut quoted = vec![b'\''];
        for &byte in &value[..end] {
            if byte == b'\'' {
                quoted.push(b'\'');
            }
            quoted.push(byte);
        }
        quoted.push(b'\'');
        PhpString::from(quoted)
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async move {
            if sql.is_empty() {
                return Err(empty_statement("exec", "statement"));
            }
            let mut connection = self.inner.lock().await;
            let outcome = connection.exec(sql).await.map_err(|e| pdo_exception(&e))?;
            Ok(if outcome.tuples {
                0
            } else {
                atol(outcome.cmd_tuples().as_bytes())
            })
        })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async move {
            let mut connection = self.inner.lock().await;
            match connection.exec("SELECT LASTVAL()").await {
                Ok(outcome) if outcome.tuples => {
                    let value = outcome
                        .rows
                        .first()
                        .and_then(|row| row.first())
                        .cloned()
                        .flatten()
                        .unwrap_or_default();
                    if value.is_empty() || value == b"0" {
                        return Err(Exception::no_identity_value(None));
                    }
                    Ok(Value::String(PhpString::from(value)))
                }
                Ok(_) => Err(pdo_exception(&PgError {
                    status: libpq::PGRES_FATAL_ERROR,
                    sql_state: None,
                    message: String::new(),
                })),
                Err(error) => {
                    let exception = pdo_exception(&error);
                    // PDO PGSQL throws a 'lastval is not yet defined in
                    // this session' error when no identity value is
                    // available, with SQLSTATE 55000 'Object Not In
                    // Prerequisite State'.
                    if error.sql_state.as_deref() == Some("55000") {
                        return Err(Exception::no_identity_value(Some(exception)));
                    }
                    Err(exception)
                }
            }
        })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut connection = self.inner.lock().await;
            if connection.in_transaction() {
                return Err(pdo_message("There is already an active transaction"));
            }
            transaction_statement(&mut connection, "BEGIN").await
        })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut connection = self.inner.lock().await;
            if !connection.in_transaction() {
                return Err(pdo_message("There is no active transaction"));
            }
            transaction_statement(&mut connection, "COMMIT").await
        })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut connection = self.inner.lock().await;
            if !connection.in_transaction() {
                return Err(pdo_message("There is no active transaction"));
            }
            transaction_statement(&mut connection, "ROLLBACK").await
        })
    }

    /// `PDO::ATTR_SERVER_VERSION`: the server's `server_version`
    /// parameter.
    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        Box::pin(async move {
            let connection = self.inner.lock().await;
            Ok(connection
                .parameter_status("server_version")
                .unwrap_or_default()
                .to_owned())
        })
    }

    /// The connection (PHP's `PDO` object): the mutex that holds the
    /// protocol connection.
    fn get_native_connection(&self) -> NativeConnection {
        Arc::new(Arc::clone(&self.inner)) as NativeConnection
    }
}

/// `Doctrine\DBAL\Driver\PDO\Statement`.
struct PdoStatement {
    inner: Arc<Mutex<PgConn>>,
    prepared: Prepared,
    /// Bound values by 0-based parameter number.
    bound: BTreeMap<usize, Param>,
}

/// `(string) $value` as PDO converts a bound value.
fn bound_string(value: &Value) -> Result<Vec<u8>, Exception> {
    if let Value::Array(_) = value {
        // "Array to string conversion" (a warning).
        return Ok(b"Array".to_vec());
    }
    value
        .to_php_string_or_stringable()
        .map(|s| s.as_bytes().to_vec())
        .ok_or_else(|| {
            let class = value
                .as_object()
                .map_or_else(|| value.debug_type(), |o| o.class().to_owned());
            Exception::new(
                Exception::ERROR,
                format!("Object of class {class} could not be converted to string"),
            )
        })
}

/// What `pdo_pgsql` sends for a value bound with a parameter type.
fn bind_param(value: &Value, r#type: ParameterType) -> Result<Param, Exception> {
    let binary = matches!(r#type, ParameterType::Binary | ParameterType::LargeObject);
    // `really_register_bound_param()`.
    let value = match (r#type, value) {
        (_, Value::Null) => Value::Null,
        (ParameterType::String | ParameterType::Ascii, other) => {
            Value::String(PhpString::from(bound_string(other)?))
        }
        (ParameterType::Integer, Value::Bool(b)) => Value::Int(i64::from(*b)),
        (ParameterType::Boolean, Value::Int(i)) => Value::Bool(*i != 0),
        _ => value.clone(),
    };
    // `pgsql_stmt_param_hook()`, `PDO_PARAM_EVT_EXEC_PRE`.
    let text = match (&value, r#type) {
        (_, ParameterType::Null) | (Value::Null, _) => None,
        (Value::Bool(b), _) => Some(if *b { b"t".to_vec() } else { b"f".to_vec() }),
        (other, _) => Some(bound_string(other)?),
    };
    Ok(Param {
        binary: binary && text.is_some(),
        value: text,
    })
}

impl DriverStatement for PdoStatement {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> Result<(), Exception> {
        let index = match param {
            Key::Int(position) => {
                if *position < 1 {
                    return Err(Exception::new(
                        Exception::VALUE_ERROR,
                        "PDOStatement::bindValue(): Argument #1 ($param) must be greater than or equal to 1",
                    ));
                }
                let index = usize::try_from(*position - 1).unwrap_or(usize::MAX);
                if self.prepared.parameter_count > 0 && index >= self.prepared.parameter_count {
                    return Err(pdo_impl_error("HY093", "parameter was not defined"));
                }
                index
            }
            Key::Str(name) => {
                let mut name = name.as_bytes().to_vec();
                if name.first() != Some(&b':') {
                    name.insert(0, b':');
                }
                match self.prepared.names.iter().find(|(known, _)| *known == name) {
                    Some((_, index)) => index - 1,
                    None => {
                        return Err(pdo_impl_error("HY093", &String::from_utf8_lossy(&name)));
                    }
                }
            }
        };
        self.bound.insert(index, bind_param(&value, r#type)?);
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            // `PQexecParams()` gets as many parameters as were bound.
            let params: Vec<Param> = (0..self.bound.len())
                .map(|index| {
                    self.bound.get(&index).cloned().unwrap_or(Param {
                        value: None,
                        binary: false,
                    })
                })
                .collect();
            let mut connection = self.inner.lock().await;
            let outcome = connection.exec_params(&self.prepared.sql, &params).await;
            outcome
                .map(|r| Box::new(result(r)) as Box<dyn DriverResult>)
                .map_err(|e| pdo_exception(&e))
        })
    }
}

/// `Doctrine\DBAL\Driver\PDO\Result`: the rows, read when the statement
/// executed.
struct PdoResult {
    columns: Vec<PhpString>,
    rows: VecDeque<Vec<Value>>,
    row_count: i64,
}

impl DriverResult for PdoResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self.rows.pop_front().map(PhpArray::list))
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self.rows.pop_front().map(|row| {
            let mut array = PhpArray::new();
            for (name, value) in self.columns.iter().zip(row) {
                array.insert(Key::from(name.clone()), value);
            }
            array
        }))
    }

    fn row_count(&self) -> Result<i64, Exception> {
        Ok(self.row_count)
    }

    fn column_count(&self) -> Result<usize, Exception> {
        Ok(self.columns.len())
    }

    fn free(&mut self) {
        self.rows.clear();
    }

    fn get_column_name(&self, index: i64) -> Option<Result<PhpString, Exception>> {
        if index < 0 {
            let value_error = Exception::new(
                Exception::VALUE_ERROR,
                "PDOStatement::getColumnMeta(): Argument #1 ($column) must be greater than or equal to 0",
            );
            return Some(Err(Exception::invalid_column_index(
                index,
                Some(value_error),
            )));
        }
        Some(
            usize::try_from(index)
                .ok()
                .and_then(|index| self.columns.get(index))
                .cloned()
                .ok_or_else(|| Exception::invalid_column_index(index, None)),
        )
    }

    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Driver\\PDO\\Result"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_become_numbered() {
        let prepared = prepare("SELECT * FROM t WHERE a = ? AND b = ? AND c = '?'").unwrap();
        assert_eq!(
            prepared.sql,
            "SELECT * FROM t WHERE a = $1 AND b = $2 AND c = '?'"
        );
        assert_eq!(prepared.parameter_count, 2);

        let prepared = prepare("SELECT :a, :b, :a").unwrap();
        assert_eq!(prepared.sql, "SELECT $1, $2, $1");
        assert_eq!(prepared.parameter_count, 2);
    }

    #[test]
    fn values_by_type() {
        assert!(matches!(
            column_value(BOOLOID, Some(b"t")),
            Value::Bool(true)
        ));
        assert!(matches!(
            column_value(INT8OID, Some(b"-42")),
            Value::Int(-42)
        ));
        assert!(
            matches!(column_value(FLOAT8OID, Some(b"1.5")), Value::Float(f) if (f - 1.5).abs() < f64::EPSILON)
        );
        assert!(
            matches!(column_value(FLOAT8OID, Some(b"-Infinity")), Value::Float(f) if f.is_infinite() && f.is_sign_negative())
        );
        assert!(
            matches!(column_value(1700, Some(b"1.50")), Value::String(s) if s.as_bytes() == b"1.50")
        );
        assert!(
            matches!(column_value(BYTEAOID, Some(b"\\x4142")), Value::String(s) if s.as_bytes() == b"AB")
        );
        assert!(matches!(column_value(INT4OID, None), Value::Null));
    }

    #[test]
    fn bound_values_by_type() {
        let text = |value: &Value, r#type| bind_param(value, r#type).unwrap().value;
        assert_eq!(
            text(&Value::Bool(true), ParameterType::String),
            Some(b"1".to_vec())
        );
        assert_eq!(
            text(&Value::Bool(true), ParameterType::Boolean),
            Some(b"t".to_vec())
        );
        assert_eq!(
            text(&Value::Int(0), ParameterType::Boolean),
            Some(b"f".to_vec())
        );
        assert_eq!(
            text(&Value::Bool(false), ParameterType::Integer),
            Some(b"0".to_vec())
        );
        assert_eq!(text(&Value::Int(5), ParameterType::Null), None);
        assert!(
            bind_param(&Value::string("x"), ParameterType::LargeObject)
                .unwrap()
                .binary
        );
    }

    #[test]
    fn messages_lose_their_trailing_newlines() {
        assert_eq!(trim_message("ERROR:  x\n"), "ERROR:  x");
        let error = PgError {
            status: 7,
            sql_state: Some("23505".to_owned()),
            message: "ERROR:  duplicate key value\nDETAIL:  Key (id)=(1) already exists.\n"
                .to_owned(),
        };
        assert_eq!(
            pdo_exception(&error).message().to_string_lossy(),
            "SQLSTATE[23505]: Unique violation: 7 ERROR:  duplicate key value\nDETAIL:  Key (id)=(1) already exists."
        );
    }
}
