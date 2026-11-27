//! The `MySQL` driver, on sqlx (the `pdo_mysql` cargo feature). It stands
//! where Doctrine's `Driver\PDO\MySQL\Driver` stands (with its
//! `PDO\Connection`, `PDO\Statement` and `PDO\Result`), and gives what
//! `pdo_mysql` (mysqlnd) gives in PHP 8.4 with its defaults:
//!
//! - Prepares are emulated, as `PDO::ATTR_EMULATE_PREPARES` is on for
//!   `pdo_mysql`: `prepare()` only reads the placeholders, and `execute()`
//!   sends the SQL with each bound value written in as a literal, over the
//!   text protocol. `NULL` (or a null value) is `NULL`; `INTEGER` writes an
//!   int (or a bool as `0`/`1`) as a number; `BOOLEAN` writes a bool or an
//!   int as `1`/`0`; any other value is the quoted `(string) $value`, and
//!   `BINARY` and `LARGE_OBJECT` quote it as `_binary'...'`.
//! - Values come back as mysqlnd gives them in PHP 8.1 and later: integer
//!   columns (`BIT` and `YEAR` included) as ints (an unsigned `BIGINT`
//!   above `PHP_INT_MAX` as a string), `FLOAT` and `DOUBLE` as floats,
//!   `NULL` as null and everything else (`DECIMAL` too) as strings.
//! - Errors read `SQLSTATE[<state>]: <description>: <code> <message>`, and
//!   for a failed connection `SQLSTATE[<state>] [<code>] <message>`; the
//!   code is `MySQL`'s error number, which the exception converter reads.
//! - `rowCount()` is the number of rows of a result set (results are
//!   buffered), and otherwise the affected rows. `lastInsertId()` is the
//!   `AUTO_INCREMENT` value of the last statement, and throws
//!   `NoIdentityValue` when it is `0`.
//! - `quote()` is `mysql_real_escape_string()` in single quotes.
//! - The connection runs no statement of its own: sqlx's session settings
//!   (`PIPES_AS_CONCAT`, `NO_ENGINE_SUBSTITUTION`, the UTC time zone) are
//!   turned off; `SET NAMES` sends the `charset` parameter (`utf8mb4` when
//!   it is not given, mysqlnd's default).
//!
//! What PDO does that this driver does not (README.md, Differences): the
//! `driverOptions` PDO attributes and `persistent` are ignored; without a
//! `host` (or with `localhost`) PDO uses the Unix socket, this driver
//! connects over TCP to `localhost` unless `unix_socket` is given; a
//! string that is not UTF-8 is written as a hex literal (`X'...'`), since
//! sqlx sends the SQL as UTF-8 text; `inTransaction()` is PDO's own flag,
//! not the server's status (an implicit commit is not seen); the column
//! names of a result without rows come from preparing the statement on
//! the server; `quote()` does not read `NO_BACKSLASH_ESCAPES`; the
//! messages of network errors are the operating system's.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/PDO/MySQL/Driver.php

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

use futures_core::Stream;
use php_value::{Key, PhpArray, PhpString, Value};
use sqlx::mysql::{MySqlConnectOptions, MySqlConnection, MySqlDatabaseError, MySqlRow};
use sqlx::{
    AssertSqlSafe, Column, Connection as _, Decode, Either, Executor, MySql, Row, SqlSafeStr,
    Statement, TypeInfo, ValueRef,
};
use tokio::sync::Mutex;

use super::api::ExceptionConverter;
use super::{
    AbstractMySQLDriver, BoxFuture, Connection as DriverConnection, Driver as DriverInterface,
    NativeConnection, PlatformBox, Result as DriverResult, Statement as DriverStatement,
};
use crate::sql::{Parser, Visitor};
use crate::{Exception, ParameterType, ServerVersionProvider};

/// `Doctrine\DBAL\Driver\PDO\MySQL\Driver`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Driver;

impl Driver {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Driver\\PDO\\MySQL\\Driver";

    pub fn new() -> Self {
        Self
    }
}

impl AbstractMySQLDriver for Driver {}

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

/// `(string) $params[$key]` as the DSN writes it.
fn param_text(params: &PhpArray, key: &str) -> Option<String> {
    isset(params, key).map(|value| {
        value
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    })
}

/// The `SQLSTATE` descriptions of `pdo_sqlstate_state_to_description()`.
fn sql_state_description(state: &str) -> &'static str {
    match state {
        "00000" => "No error",
        "01000" => "Warning",
        "07001" => "Wrong number of parameters",
        "08001" => "Client unable to establish connection",
        "08003" => "Connection does not exist",
        "08004" => "Server rejected the connection",
        "08S01" => "Communication link failure",
        "0A000" => "Feature not supported",
        "21000" => "Cardinality violation",
        "21S01" => "Insert value list does not match column list",
        "21S02" => "Degree of derived table does not match column list",
        "01004" | "22001" => "String data, right truncated",
        "22003" => "Numeric value out of range",
        "22007" => "Invalid datetime format",
        "22008" => "Datetime field overflow",
        "22012" => "Division by zero",
        "22018" => "Invalid character value for cast specification",
        "23000" => "Integrity constraint violation",
        "24000" => "Invalid cursor state",
        "25000" => "Invalid transaction state",
        "28000" => "Invalid authorization specification",
        "3D000" => "Invalid catalog name",
        "3F000" => "Invalid schema name",
        "40001" => "Serialization failure",
        "42000" => "Syntax error or access violation",
        "42S01" => "Base table or view already exists",
        "42S02" => "Base table or view not found",
        "42S11" => "Index already exists",
        "42S12" => "Index not found",
        "42S21" => "Column already exists",
        "42S22" => "Column not found",
        "44000" => "WITH CHECK OPTION violation",
        "HY000" => "General error",
        "HY001" => "Memory allocation error",
        "HY093" => "Invalid parameter number",
        "HYT00" => "Timeout expired",
        "IM001" => "Driver does not support this function",
        _ => "<<Unknown error>>",
    }
}

/// The `SQLSTATE`, error number and message of what sqlx returned: a
/// server error as `MySQL` gives it, a network error as mysqlnd's client
/// error `code`.
fn mysql_error(error: &sqlx::Error, network_code: i64) -> (String, i64, String) {
    match error {
        sqlx::Error::Database(database) => {
            let code = database
                .try_downcast_ref::<MySqlDatabaseError>()
                .map_or(0, |e| i64::from(e.number()));
            let state = database
                .code()
                .map_or_else(|| "HY000".to_owned(), std::borrow::Cow::into_owned);
            (state, code, database.message().to_owned())
        }
        sqlx::Error::Io(io) => {
            let message = io.to_string();
            // The operating system's message, without Rust's "(os error N)".
            let message = match message.find(" (os error ") {
                Some(end) => message[..end].to_owned(),
                None => message,
            };
            let message = if network_code == 2006 {
                "MySQL server has gone away".to_owned()
            } else {
                message
            };
            ("HY000".to_owned(), network_code, message)
        }
        other => ("HY000".to_owned(), 0, other.to_string()),
    }
}

/// `PDO\Exception::new($pdoException)` for an error of a statement.
fn statement_error(state: &str, code: i64, message: &str) -> Exception {
    Exception::driver(
        Exception::PDO_EXCEPTION,
        format!(
            "SQLSTATE[{state}]: {}: {code} {message}",
            sql_state_description(state)
        ),
        Some(state.to_owned()),
        code,
    )
}

/// `PDO\Exception::new($pdoException)` for what sqlx returned.
fn pdo_exception(error: &sqlx::Error) -> Exception {
    let (state, code, message) = mysql_error(error, 2006);
    statement_error(&state, code, &message)
}

/// `PDO\Exception::new($pdoException)` for an error of `new PDO()`.
fn connect_error(error: &sqlx::Error) -> Exception {
    let (state, code, message) = mysql_error(error, 2002);
    Exception::driver(
        Exception::PDO_EXCEPTION,
        format!("SQLSTATE[{state}] [{code}] {message}"),
        Some(state),
        code,
    )
}

/// An error PDO raises itself, with an `SQLSTATE` and no driver code.
fn pdo_error(state: &str, detail: &str) -> Exception {
    Exception::driver(
        Exception::PDO_EXCEPTION,
        format!(
            "SQLSTATE[{state}]: {}: {detail}",
            sql_state_description(state)
        ),
        Some(state.to_owned()),
        0,
    )
}

/// A PDO error raised outside `MySQL` (no `SQLSTATE`).
fn pdo_message(message: &str) -> Exception {
    Exception::driver(Exception::PDO_EXCEPTION, message, None, 0)
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
            // `constructPdoDsn()`, read into sqlx's options.
            let mut options = MySqlConnectOptions::new()
                .pipes_as_concat(false)
                .no_engine_substitution(false)
                .timezone(None)
                .statement_cache_capacity(0)
                .username(&param_text(params, "user").unwrap_or_default());
            if let Some(host) = param_text(params, "host").filter(|host| !host.is_empty()) {
                options = options.host(&host);
            }
            if let Some(port) = param_text(params, "port") {
                // The DSN's `port=` is read with `atoi()`.
                let digits: String = port
                    .trim()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                options = options.port(digits.parse().unwrap_or(3306));
            }
            if let Some(dbname) = param_text(params, "dbname") {
                options = options.database(&dbname);
            }
            if let Some(socket) = param_text(params, "unix_socket") {
                options = options.socket(socket);
            }
            if let Some(charset) = param_text(params, "charset") {
                options = options.charset(&charset);
            }
            if let Some(password) = param_text(params, "password").filter(|p| !p.is_empty()) {
                options = options.password(&password);
            }
            let mut connection = MySqlConnection::connect_with(&options)
                .await
                .map_err(|e| connect_error(&e))?;
            let server_version = sqlx::query_scalar::<MySql, String>("SELECT VERSION()")
                .fetch_one(&mut connection)
                .await
                .map_err(|e| connect_error(&e))?;
            Ok(Arc::new(PdoConnection {
                inner: Arc::new(Mutex::new(Inner {
                    connection,
                    in_transaction: false,
                    last_insert_id: 0,
                })),
                server_version,
            }) as Arc<dyn DriverConnection>)
        })
    }

    fn get_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        self.mysql_database_platform(version_provider)
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        self.mysql_exception_converter()
    }
}

/// The `MySQL` connection (PHP's `PDO` object).
pub struct Inner {
    connection: MySqlConnection,
    /// PDO's own transaction flag.
    in_transaction: bool,
    /// `mysql_insert_id()`: the `AUTO_INCREMENT` value of the last
    /// statement.
    last_insert_id: u64,
}

/// `Doctrine\DBAL\Driver\PDO\Connection` on a `MySQL` database.
struct PdoConnection {
    inner: Arc<Mutex<Inner>>,
    /// `PDO::ATTR_SERVER_VERSION`, read when connecting.
    server_version: String,
}

/// A piece of a prepared statement's SQL.
#[derive(Debug, Clone)]
enum Piece {
    Sql(Vec<u8>),
    /// A `?`, by its position (from 1).
    Positional(usize),
    /// A `:name` (with its colon).
    Named(Vec<u8>),
}

/// The SQL cut at its placeholders, as PDO's emulation reads it.
#[derive(Default)]
struct Placeholders {
    pieces: Vec<Piece>,
    positional: usize,
    named: bool,
}

impl Visitor for Placeholders {
    type Error = std::convert::Infallible;

    fn accept_positional_parameter(&mut self, _sql: &[u8]) -> Result<(), Self::Error> {
        self.positional += 1;
        self.pieces.push(Piece::Positional(self.positional));
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.named = true;
        self.pieces.push(Piece::Named(sql.to_vec()));
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        if let Some(Piece::Sql(last)) = self.pieces.last_mut() {
            last.extend_from_slice(sql);
        } else {
            self.pieces.push(Piece::Sql(sql.to_vec()));
        }
        Ok(())
    }
}

/// `mysql_real_escape_string()`.
fn escape(value: &[u8]) -> Vec<u8> {
    let mut escaped = Vec::with_capacity(value.len() + 2);
    for &byte in value {
        match byte {
            0 => escaped.extend_from_slice(b"\\0"),
            b'\n' => escaped.extend_from_slice(b"\\n"),
            b'\r' => escaped.extend_from_slice(b"\\r"),
            b'\\' => escaped.extend_from_slice(b"\\\\"),
            b'\'' => escaped.extend_from_slice(b"\\'"),
            b'"' => escaped.extend_from_slice(b"\\\""),
            0x1a => escaped.extend_from_slice(b"\\Z"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// `PDO::quote()`: the escaped value in single quotes.
fn quote(value: &[u8]) -> Vec<u8> {
    let mut quoted = vec![b'\''];
    quoted.extend(escape(value));
    quoted.push(b'\'');
    quoted
}

/// A literal of a value that sqlx can send in UTF-8 SQL: the quoted
/// string, or a hex literal when the bytes are not UTF-8.
fn string_literal(value: &[u8], binary: bool) -> String {
    if std::str::from_utf8(value).is_ok() {
        let mut literal = if binary {
            b"_binary".to_vec()
        } else {
            Vec::new()
        };
        literal.extend(quote(value));
        String::from_utf8(literal).unwrap_or_default()
    } else {
        let mut hex = String::from("X'");
        for byte in value {
            let _ = write!(hex, "{byte:02X}");
        }
        hex.push('\'');
        hex
    }
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

/// The literal PDO's emulation writes for a bound value.
fn literal(value: &Value, r#type: ParameterType) -> Result<String, Exception> {
    if value.is_null() || r#type == ParameterType::Null {
        return Ok("NULL".to_owned());
    }
    Ok(match (r#type, value) {
        (ParameterType::Integer, Value::Int(i)) => i.to_string(),
        (ParameterType::Integer | ParameterType::Boolean, Value::Bool(b)) => {
            if *b { "1" } else { "0" }.to_owned()
        }
        (ParameterType::Boolean, Value::Int(i)) => if *i == 0 { "0" } else { "1" }.to_owned(),
        (ParameterType::Binary | ParameterType::LargeObject, _) => {
            string_literal(&bound_string(value)?, true)
        }
        _ => string_literal(&bound_string(value)?, false),
    })
}

/// What running a statement gives: the first result set (or none), the
/// affected rows, and the columns.
struct Ran {
    columns: Vec<(PhpString, String)>,
    rows: VecDeque<Vec<Value>>,
    affected_rows: u64,
}

/// A value read from `MySQL` over the text protocol, as mysqlnd gives it.
fn column_value(row: &MySqlRow, index: usize, type_name: &str) -> Result<Value, Exception> {
    let raw = row.try_get_raw(index).map_err(|e| pdo_exception(&e))?;
    if raw.is_null() {
        return Ok(Value::Null);
    }
    let bytes = <&[u8] as Decode<MySql>>::decode(raw)
        .map_err(|e| Exception::driver(Exception::PDO_EXCEPTION, e.to_string(), None, 0))?;
    let text = || String::from_utf8_lossy(bytes).into_owned();
    let integer = |text: String| match text.parse::<i64>() {
        Ok(i) => Value::Int(i),
        Err(_) => Value::String(PhpString::from(text)),
    };
    let base = type_name.trim_end_matches(" UNSIGNED");
    Ok(match base {
        "BOOLEAN" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" | "YEAR" => {
            integer(text())
        }
        "FLOAT" | "DOUBLE" => match text().parse::<f64>() {
            Ok(f) => Value::Float(f),
            Err(_) => Value::String(PhpString::from(bytes.to_vec())),
        },
        "BIT" => {
            let number = bytes.iter().fold(0u64, |n, &b| (n << 8) | u64::from(b));
            integer(number.to_string())
        }
        _ => Value::String(PhpString::from(bytes.to_vec())),
    })
}

/// Waits for the next item of a stream.
async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx)).await
}

/// Runs SQL over the text protocol and reads its first result set.
async fn run(inner: &mut Inner, sql: String) -> Result<Ran, Exception> {
    inner.last_insert_id = 0;
    let mut columns: Option<Vec<(PhpString, String)>> = None;
    let mut rows = VecDeque::new();
    let mut affected_rows = None;
    let mut last_insert_id = 0;
    {
        let mut stream =
            (&mut inner.connection).fetch_many(sqlx::raw_sql(AssertSqlSafe(sql.clone())));
        while let Some(item) = next(&mut stream).await {
            match item.map_err(|e| pdo_exception(&e))? {
                Either::Left(done) => {
                    last_insert_id = done.last_insert_id();
                    if affected_rows.is_none() {
                        affected_rows = Some(done.rows_affected());
                    }
                }
                Either::Right(row) if affected_rows.is_none() => {
                    let names = columns.get_or_insert_with(|| {
                        row.columns()
                            .iter()
                            .map(|c| (PhpString::from(c.name()), c.type_info().name().to_owned()))
                            .collect()
                    });
                    let mut values = Vec::with_capacity(names.len());
                    for (index, (_, type_name)) in names.iter().enumerate() {
                        values.push(column_value(&row, index, type_name)?);
                    }
                    rows.push_back(values);
                }
                Either::Right(_) => {}
            }
        }
    }
    inner.last_insert_id = last_insert_id;
    let affected_rows = affected_rows.unwrap_or(0);
    let columns = match columns {
        Some(columns) => columns,
        // A result set without rows has its columns in the metadata sqlx
        // does not keep: prepare the statement for them (it does not run).
        None if affected_rows == 0 => match (&mut inner.connection)
            .prepare(AssertSqlSafe(sql).into_sql_str())
            .await
        {
            Ok(statement) => statement
                .columns()
                .iter()
                .map(|c| (PhpString::from(c.name()), c.type_info().name().to_owned()))
                .collect(),
            Err(_) => Vec::new(),
        },
        None => Vec::new(),
    };
    Ok(Ran {
        columns,
        rows,
        affected_rows,
    })
}

/// A result of what `run()` read.
fn result(ran: Ran) -> Box<dyn DriverResult> {
    let row_count = if ran.columns.is_empty() {
        i64::try_from(ran.affected_rows).unwrap_or(i64::MAX)
    } else {
        i64::try_from(ran.rows.len()).unwrap_or(i64::MAX)
    };
    Box::new(PdoResult {
        columns: ran.columns.into_iter().map(|(name, _)| name).collect(),
        rows: ran.rows,
        row_count,
        freed: false,
    })
}

/// PDO's `ValueError` for an empty statement.
fn empty_statement(method: &str, parameter: &str) -> Exception {
    Exception::new(
        Exception::VALUE_ERROR,
        format!("PDO::{method}(): Argument #1 (${parameter}) must not be empty"),
    )
}

/// SQL that sqlx can send.
fn utf8(sql: Vec<u8>) -> Result<String, Exception> {
    String::from_utf8(sql)
        .map_err(|_| Exception::new(Exception::UNSUPPORTED, "SQL that is not UTF-8"))
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
            let mut placeholders = Placeholders::default();
            let Ok(()) = Parser::new(true).parse(sql.as_bytes(), &mut placeholders);
            if placeholders.named && placeholders.positional > 0 {
                return Err(pdo_error("HY093", "mixed named and positional parameters"));
            }
            Ok(Box::new(PdoStatement {
                inner: Arc::clone(&self.inner),
                pieces: placeholders.pieces,
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
            let mut inner = self.inner.lock().await;
            Ok(result(run(&mut inner, sql.to_owned()).await?))
        })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        PhpString::from(quote(value))
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async move {
            if sql.is_empty() {
                return Err(empty_statement("exec", "statement"));
            }
            let mut inner = self.inner.lock().await;
            let ran = run(&mut inner, sql.to_owned()).await?;
            Ok(if ran.columns.is_empty() {
                i64::try_from(ran.affected_rows).unwrap_or(i64::MAX)
            } else {
                i64::try_from(ran.rows.len()).unwrap_or(i64::MAX)
            })
        })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async move {
            let inner = self.inner.lock().await;
            // pdo_mysql returns '0' when there is none.
            if inner.last_insert_id == 0 {
                return Err(Exception::no_identity_value(None));
            }
            Ok(Value::String(PhpString::from(
                inner.last_insert_id.to_string(),
            )))
        })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            if inner.in_transaction {
                return Err(pdo_message("There is already an active transaction"));
            }
            run(&mut inner, "START TRANSACTION".to_owned()).await?;
            inner.in_transaction = true;
            Ok(())
        })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            if !inner.in_transaction {
                return Err(pdo_message("There is no active transaction"));
            }
            run(&mut inner, "COMMIT".to_owned()).await?;
            inner.in_transaction = false;
            Ok(())
        })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            if !inner.in_transaction {
                return Err(pdo_message("There is no active transaction"));
            }
            inner.in_transaction = false;
            run(&mut inner, "ROLLBACK".to_owned()).await?;
            Ok(())
        })
    }

    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        Box::pin(async move { Ok(self.server_version.clone()) })
    }

    /// The `MySQL` connection (PHP's `PDO` object): the mutex that holds
    /// the sqlx connection and PDO's state.
    fn get_native_connection(&self) -> NativeConnection {
        Arc::clone(&self.inner) as NativeConnection
    }
}

/// `Doctrine\DBAL\Driver\PDO\Statement`, with emulated prepares.
struct PdoStatement {
    inner: Arc<Mutex<Inner>>,
    pieces: Vec<Piece>,
    /// Bound values by parameter.
    bound: BTreeMap<Key, (Value, ParameterType)>,
}

impl DriverStatement for PdoStatement {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> Result<(), Exception> {
        let key = match param {
            Key::Int(position) => {
                if *position < 1 {
                    return Err(Exception::new(
                        Exception::VALUE_ERROR,
                        "PDOStatement::bindValue(): Argument #1 ($param) must be greater than or equal to 1",
                    ));
                }
                Key::Int(*position)
            }
            Key::Str(name) => {
                let mut name = name.as_bytes().to_vec();
                if name.first() != Some(&b':') {
                    name.insert(0, b':');
                }
                let known = self
                    .pieces
                    .iter()
                    .any(|piece| matches!(piece, Piece::Named(n) if *n == name));
                if !known {
                    return Err(pdo_error("HY093", "parameter was not defined"));
                }
                Key::from(PhpString::from(name))
            }
        };
        self.bound.insert(key, (value, r#type));
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            let mut sql = Vec::new();
            for piece in &self.pieces {
                let key = match piece {
                    Piece::Sql(text) => {
                        sql.extend_from_slice(text);
                        continue;
                    }
                    Piece::Positional(position) => {
                        Key::Int(i64::try_from(*position).unwrap_or(i64::MAX))
                    }
                    Piece::Named(name) => Key::from(PhpString::from(name.clone())),
                };
                let Some((value, r#type)) = self.bound.get(&key) else {
                    return Err(pdo_error(
                        "HY093",
                        "number of bound variables does not match number of tokens",
                    ));
                };
                sql.extend_from_slice(literal(value, *r#type)?.as_bytes());
            }
            let positional = self
                .pieces
                .iter()
                .filter(|piece| matches!(piece, Piece::Positional(_)))
                .count();
            if self
                .bound
                .keys()
                .any(|key| matches!(key, Key::Int(i) if usize::try_from(*i).map_or(true, |i| i > positional)))
            {
                return Err(pdo_error(
                    "HY093",
                    "number of bound variables does not match number of tokens",
                ));
            }
            let sql = utf8(sql)?;
            let mut inner = self.inner.lock().await;
            Ok(result(run(&mut inner, sql).await?))
        })
    }
}

/// `Doctrine\DBAL\Driver\PDO\Result`: the rows, read when the statement
/// executed (mysqlnd buffers them).
struct PdoResult {
    columns: Vec<PhpString>,
    rows: VecDeque<Vec<Value>>,
    row_count: i64,
    /// `free()` closed the cursor.
    freed: bool,
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
        self.freed = true;
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
        if self.freed {
            return Some(Err(Exception::invalid_column_index(index, None)));
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
