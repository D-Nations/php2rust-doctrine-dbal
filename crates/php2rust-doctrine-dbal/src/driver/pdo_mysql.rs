//! The `MySQL` driver, on sqlx (the `pdo_mysql` cargo feature). It stands
//! where Doctrine's `Driver\PDO\MySQL\Driver` stands (with its
//! `PDO\Connection`, `PDO\Statement` and `PDO\Result`), and gives what
//! `pdo_mysql` (mysqlnd) gives in PHP 8.4 with its defaults:
//!
//! - Prepares are the server's (`PDO::ATTR_EMULATE_PREPARES` off): the
//!   values are bound through sqlx, as this repository binds every value
//!   (PDO's default for `pdo_mysql` is to write them into the SQL).
//!   `prepare()` prepares the statement on the server (its errors come
//!   from there); a named placeholder becomes a `?` (one name used twice
//!   binds its value twice). A value converts as PDO converts it for its
//!   type: `NULL` (or a null value) binds null; `INTEGER` and `BOOLEAN`
//!   bind `(int) $value` (`BOOLEAN` makes an int 0 or 1 first); the other
//!   types bind `(string) $value`, as text, or as bytes for `BINARY`,
//!   `LARGE_OBJECT` and a string that is not UTF-8. `query()` and `exec()`
//!   send the SQL over the text protocol.
//! - Values come back as mysqlnd gives them in PHP 8.1 and later: integer
//!   columns (`BIT` and `YEAR` included) as ints (an unsigned `BIGINT`
//!   above `PHP_INT_MAX` as a string), `FLOAT` and `DOUBLE` as floats
//!   (a `FLOAT` read from a prepared statement rounded to 6 significant
//!   digits, mysqlnd's `FLT_DIG`), `NULL` as null and everything else
//!   (`DECIMAL`, dates and times too) as strings.
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
//! connects over TCP to `localhost` unless `unix_socket` is given;
//! `inTransaction()` is PDO's own flag, not the server's status (an
//! implicit commit is not seen); the column names of a result of
//! `query()` without rows come from preparing the statement on the
//! server; a date or time of a prepared statement shows its fraction of a
//! second with 6 digits when it has one (mysqlnd uses the column's
//! precision, which sqlx does not give); `quote()` does not read
//! `NO_BACKSLASH_ESCAPES`; the messages of network errors are the
//! operating system's.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/PDO/MySQL/Driver.php

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::sync::Arc;

use futures_core::Stream;
use php_value::{Key, PhpArray, PhpString, Value};
use sqlx::mysql::{
    MySqlArguments, MySqlConnectOptions, MySqlConnection, MySqlDatabaseError, MySqlRow,
};
use sqlx::{
    Arguments, AssertSqlSafe, Column, Connection as _, Decode, Either, Executor, MySql, Row,
    SqlSafeStr, Statement, TypeInfo, ValueRef,
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

/// A `?` of the SQL sent to the server: the parameter it binds.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Placeholder {
    /// A `?`, by its position (from 1).
    Positional(i64),
    /// A `:name` (with its colon), rewritten as `?`.
    Named(Vec<u8>),
}

/// The SQL with its named placeholders rewritten as `?`, as PDO rewrites
/// them for the server.
#[derive(Default)]
struct Placeholders {
    sql: Vec<u8>,
    list: Vec<Placeholder>,
    positional: i64,
    named: bool,
}

impl Visitor for Placeholders {
    type Error = std::convert::Infallible;

    fn accept_positional_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.positional += 1;
        self.list.push(Placeholder::Positional(self.positional));
        self.sql.extend_from_slice(sql);
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.named = true;
        self.list.push(Placeholder::Named(sql.to_vec()));
        self.sql.push(b'?');
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.sql.extend_from_slice(sql);
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

/// Adds a bound value to the arguments, converted as PDO converts it for
/// its type.
fn add_argument(
    arguments: &mut MySqlArguments,
    value: &Value,
    r#type: ParameterType,
) -> Result<(), Exception> {
    let added = if value.is_null() || r#type == ParameterType::Null {
        arguments.add(Option::<i64>::None)
    } else {
        match r#type {
            ParameterType::Null => arguments.add(Option::<i64>::None),
            ParameterType::Integer => arguments.add(php_value::numeric::int_cast(value)),
            ParameterType::Boolean => arguments.add(match value {
                Value::Int(i) => i64::from(*i != 0),
                other => php_value::numeric::int_cast(other),
            }),
            ParameterType::String | ParameterType::Ascii => {
                match String::from_utf8(bound_string(value)?) {
                    Ok(text) => arguments.add(text),
                    Err(bytes) => arguments.add(bytes.into_bytes()),
                }
            }
            ParameterType::Binary | ParameterType::LargeObject => {
                arguments.add(bound_string(value)?)
            }
        }
    };
    added.map_err(|e| Exception::driver(Exception::PDO_EXCEPTION, e.to_string(), None, 0))
}

/// What running a statement gives: the first result set (or none), the
/// affected rows, and the columns.
struct Ran {
    columns: Vec<(PhpString, String)>,
    rows: VecDeque<Vec<Value>>,
    affected_rows: u64,
}

/// A value read from `MySQL`, as mysqlnd gives it: from the text protocol
/// (`query()`, `exec()`) or the binary one (a prepared statement).
fn column_value(
    row: &MySqlRow,
    index: usize,
    type_name: &str,
    binary: bool,
) -> Result<Value, Exception> {
    let raw = row.try_get_raw(index).map_err(|e| pdo_exception(&e))?;
    if raw.is_null() {
        return Ok(Value::Null);
    }
    let bytes = <&[u8] as Decode<MySql>>::decode(raw)
        .map_err(|e| Exception::driver(Exception::PDO_EXCEPTION, e.to_string(), None, 0))?;
    if binary {
        return Ok(binary_value(bytes, type_name));
    }
    let text = || String::from_utf8_lossy(bytes).into_owned();
    let base = type_name.trim_end_matches(" UNSIGNED");
    Ok(match base {
        "BOOLEAN" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" | "YEAR" => {
            integer(text())
        }
        "FLOAT" | "DOUBLE" => match text().parse::<f64>() {
            Ok(f) => Value::Float(f),
            Err(_) => Value::String(PhpString::from(bytes.to_vec())),
        },
        "BIT" => integer(big_endian(bytes).to_string()),
        _ => Value::String(PhpString::from(bytes.to_vec())),
    })
}

/// An integer, or its text when it does not fit (an unsigned `BIGINT`).
fn integer(text: String) -> Value {
    match text.parse::<i64>() {
        Ok(i) => Value::Int(i),
        Err(_) => Value::String(PhpString::from(text)),
    }
}

/// A `BIT` value's bytes as a number.
fn big_endian(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0u64, |n, &b| (n << 8) | u64::from(b))
}

/// The little-endian integer of `bytes`.
fn little_endian(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .rev()
        .fold(0u64, |n, &b| (n << 8) | u64::from(b))
}

/// A value of the binary protocol, as mysqlnd's `ps_fetch_*()` gives it.
fn binary_value(bytes: &[u8], type_name: &str) -> Value {
    let unsigned = type_name.ends_with(" UNSIGNED");
    let base = type_name.trim_end_matches(" UNSIGNED");
    let signed = |width: usize| {
        let n = little_endian(bytes);
        let shift = 64 - 8 * u32::try_from(width.clamp(1, 8)).unwrap_or(8);
        // Sign extension from the column's width.
        #[allow(clippy::cast_possible_wrap, reason = "two's complement of the width")]
        let value = ((n << shift) as i64) >> shift;
        Value::Int(value)
    };
    match base {
        "BOOLEAN" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" if unsigned => {
            integer(little_endian(bytes).to_string())
        }
        "BOOLEAN" | "TINYINT" | "SMALLINT" | "MEDIUMINT" | "INT" | "BIGINT" => signed(bytes.len()),
        "YEAR" => integer(little_endian(bytes).to_string()),
        "FLOAT" => {
            let value = bytes
                .get(..4)
                .and_then(|b| <[u8; 4]>::try_from(b).ok())
                .map_or(0.0, f32::from_le_bytes);
            // `mysql_float_to_double()` with `FLT_DIG` (6) digits.
            Value::Float(format!("{value:.5e}").parse().unwrap_or(f64::from(value)))
        }
        "DOUBLE" => Value::Float(
            bytes
                .get(..8)
                .and_then(|b| <[u8; 8]>::try_from(b).ok())
                .map_or(0.0, f64::from_le_bytes),
        ),
        "BIT" => integer(big_endian(bytes).to_string()),
        "DATE" | "DATETIME" | "TIMESTAMP" => Value::String(PhpString::from(date_time(bytes, base))),
        "TIME" => Value::String(PhpString::from(time(bytes))),
        _ => Value::String(PhpString::from(bytes.to_vec())),
    }
}

/// A binary `DATE`, `DATETIME` or `TIMESTAMP` (its length byte first).
fn date_time(bytes: &[u8], base: &str) -> String {
    let at = |i: usize| bytes.get(i).copied().map_or(0, u32::from);
    let year = at(1) | (at(2) << 8);
    let date = format!("{year:04}-{:02}-{:02}", at(3), at(4));
    if base == "DATE" {
        return date;
    }
    let mut text = format!("{date} {:02}:{:02}:{:02}", at(5), at(6), at(7));
    let micro = at(8) | (at(9) << 8) | (at(10) << 16) | (at(11) << 24);
    if micro != 0 {
        let _ = write!(text, ".{micro:06}");
    }
    text
}

/// A binary `TIME` (its length byte first): `[-]HH:MM:SS`, the days
/// counted in the hours.
fn time(bytes: &[u8]) -> String {
    let at = |i: usize| bytes.get(i).copied().map_or(0, u32::from);
    let negative = at(1) == 1;
    let days = at(2) | (at(3) << 8) | (at(4) << 16) | (at(5) << 24);
    let hours = days * 24 + at(6);
    let mut text = format!(
        "{}{hours:02}:{:02}:{:02}",
        if negative { "-" } else { "" },
        at(7),
        at(8)
    );
    let micro = at(9) | (at(10) << 8) | (at(11) << 16) | (at(12) << 24);
    if micro != 0 {
        let _ = write!(text, ".{micro:06}");
    }
    text
}

/// Waits for the next item of a stream.
async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    std::future::poll_fn(|cx| std::pin::Pin::new(&mut *stream).poll_next(cx)).await
}

/// Runs SQL and reads its first result set: over the text protocol, or
/// as a prepared statement with `arguments` (whose `columns` it knows).
async fn run(
    inner: &mut Inner,
    sql: String,
    arguments: Option<(MySqlArguments, Vec<(PhpString, String)>)>,
) -> Result<Ran, Exception> {
    let binary = arguments.is_some();
    let (arguments, known_columns) = match arguments {
        Some((arguments, columns)) => (Some(arguments), Some(columns)),
        None => (None, None),
    };
    inner.last_insert_id = 0;
    let mut columns: Option<Vec<(PhpString, String)>> = None;
    let mut rows = VecDeque::new();
    let mut affected_rows = None;
    let mut last_insert_id = 0;
    {
        let mut stream = match arguments {
            Some(arguments) => (&mut inner.connection)
                .fetch_many(sqlx::query_with(AssertSqlSafe(sql.clone()), arguments)),
            None => (&mut inner.connection).fetch_many(sqlx::raw_sql(AssertSqlSafe(sql.clone()))),
        };
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
                        values.push(column_value(&row, index, type_name, binary)?);
                    }
                    rows.push_back(values);
                }
                Either::Right(_) => {}
            }
        }
    }
    inner.last_insert_id = last_insert_id;
    let affected_rows = affected_rows.unwrap_or(0);
    let columns = match columns.or(known_columns) {
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
            let server_sql = utf8(placeholders.sql)?;
            let mut inner = self.inner.lock().await;
            let statement = (&mut inner.connection)
                .prepare(AssertSqlSafe(server_sql.clone()).into_sql_str())
                .await
                .map_err(|e| pdo_exception(&e))?;
            let columns = statement
                .columns()
                .iter()
                .map(|c| (PhpString::from(c.name()), c.type_info().name().to_owned()))
                .collect();
            Ok(Box::new(PdoStatement {
                inner: Arc::clone(&self.inner),
                sql: server_sql,
                placeholders: placeholders.list,
                columns,
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
            Ok(result(run(&mut inner, sql.to_owned(), None).await?))
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
            let ran = run(&mut inner, sql.to_owned(), None).await?;
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
            run(&mut inner, "START TRANSACTION".to_owned(), None).await?;
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
            run(&mut inner, "COMMIT".to_owned(), None).await?;
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
            run(&mut inner, "ROLLBACK".to_owned(), None).await?;
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

/// `Doctrine\DBAL\Driver\PDO\Statement`, prepared on the server.
struct PdoStatement {
    inner: Arc<Mutex<Inner>>,
    /// The SQL sent to the server (named placeholders as `?`).
    sql: String,
    placeholders: Vec<Placeholder>,
    /// The result columns the server described.
    columns: Vec<(PhpString, String)>,
    /// Bound values by parameter.
    bound: BTreeMap<Key, (Value, ParameterType)>,
}

/// PDO's error for parameters that do not match the placeholders.
fn parameter_mismatch() -> Exception {
    pdo_error(
        "HY093",
        "number of bound variables does not match number of tokens",
    )
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
                if !self
                    .placeholders
                    .contains(&Placeholder::Named(name.clone()))
                {
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
            let positional = self
                .placeholders
                .iter()
                .filter(|p| matches!(p, Placeholder::Positional(_)))
                .count();
            let past_the_end = self.bound.keys().any(|key| {
                matches!(key, Key::Int(i) if usize::try_from(*i).map_or(true, |i| i > positional))
            });
            if past_the_end {
                return Err(parameter_mismatch());
            }
            let mut arguments = MySqlArguments::default();
            for placeholder in &self.placeholders {
                let key = match placeholder {
                    Placeholder::Positional(position) => Key::Int(*position),
                    Placeholder::Named(name) => Key::from(PhpString::from(name.clone())),
                };
                let Some((value, r#type)) = self.bound.get(&key) else {
                    return Err(parameter_mismatch());
                };
                add_argument(&mut arguments, value, *r#type)?;
            }
            let mut inner = self.inner.lock().await;
            Ok(result(
                run(
                    &mut inner,
                    self.sql.clone(),
                    Some((arguments, self.columns.clone())),
                )
                .await?,
            ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_binary_values_as_mysqlnd() {
        assert!(Value::Int(-2).identical(&binary_value(&[0xfe], "TINYINT")));
        assert!(Value::Int(254).identical(&binary_value(&[0xfe], "TINYINT UNSIGNED")));
        assert!(Value::Int(-1).identical(&binary_value(&[0xff; 8], "BIGINT")));
        assert!(
            Value::string("18446744073709551615")
                .identical(&binary_value(&[0xff; 8], "BIGINT UNSIGNED"))
        );
        assert!(Value::Int(2024).identical(&binary_value(&[0xe8, 0x07], "YEAR")));
        assert!(Value::Float(1.1).identical(&binary_value(&1.1f32.to_le_bytes(), "FLOAT")));
        assert!(Value::Int(5).identical(&binary_value(&[0, 5], "BIT")));
        assert!(
            Value::string("2012-12-12").identical(&binary_value(&[4, 0xdc, 0x07, 12, 12], "DATE"))
        );
        assert!(
            Value::string("2012-12-12 01:02:03")
                .identical(&binary_value(&[7, 0xdc, 0x07, 12, 12, 1, 2, 3], "DATETIME"))
        );
        assert!(Value::string("0000-00-00 00:00:00").identical(&binary_value(&[0], "TIMESTAMP")));
        assert!(
            Value::string("-25:02:03")
                .identical(&binary_value(&[8, 1, 1, 0, 0, 0, 1, 2, 3], "TIME"))
        );
        assert!(Value::string("-2.300").identical(&binary_value(b"-2.300", "DECIMAL")));
    }

    #[test]
    fn rewrites_named_placeholders() {
        let mut placeholders = Placeholders::default();
        let Ok(()) = Parser::new(true).parse(b"SELECT :a, ':b', :a", &mut placeholders);
        assert_eq!(b"SELECT ?, ':b', ?".to_vec(), placeholders.sql);
        assert_eq!(
            vec![
                Placeholder::Named(b":a".to_vec()),
                Placeholder::Named(b":a".to_vec())
            ],
            placeholders.list
        );
    }

    #[test]
    fn quotes_as_mysql_real_escape_string() {
        assert_eq!(b"'a\\'b\\\\c\\0\\n'".to_vec(), quote(b"a'b\\c\0\n"));
    }
}
