//! What `pdo_pgsql` asks of libpq, on the `PostgreSQL` frontend/backend
//! protocol (version 3.0, the messages of the `postgres-protocol` crate):
//!
//! - `PQconnectdb()`: a TCP connection to each address of the host (or
//!   the Unix socket of a host that starts with `/`), the startup message
//!   (`user`, `database`, `application_name`), cleartext, MD5 and
//!   SCRAM-SHA-256 authentication, and the server's parameters
//!   (`server_version`, `standard_conforming_strings`, ...). There is no
//!   TLS: `sslmode` `disable`, `allow` and `prefer` connect in plain text
//!   (as libpq does with a server that refuses TLS), the other modes are
//!   refused.
//! - `PQexec()`: the simple query protocol, several statements allowed;
//!   the last result is kept.
//! - `PQexecParams()`: the extended protocol on the unnamed statement:
//!   parameters of unknown type (OID 0), sent as text (format 0) or as
//!   bytes (format 1), and results as text.
//! - `PQerrorMessage()`: libpq's message of an error (`ERROR:  ...`, the
//!   `LINE n:` cursor display of the statement position, `DETAIL:`,
//!   `HINT:`, `QUERY:`, `CONTEXT:`), and its connection messages.
//! - `PQtransactionStatus()`, `PQcmdTuples()`, `PQunescapeBytea()` and
//!   `PQescapeStringConn()`.
//!
//! Upstream: none

use std::fmt::Write as _;
use std::io;
use std::time::Duration;

use bytes::BytesMut;
use fallible_iterator::FallibleIterator;
use postgres_protocol::authentication::md5_hash;
use postgres_protocol::authentication::sasl::{ChannelBinding, SCRAM_SHA_256, ScramSha256};
use postgres_protocol::message::backend::{ErrorResponseBody, Message, RowDescriptionBody};
use postgres_protocol::message::frontend;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// `PGRES_EMPTY_QUERY`.
pub(super) const PGRES_EMPTY_QUERY: i64 = 0;
/// `PGRES_FATAL_ERROR`.
pub(super) const PGRES_FATAL_ERROR: i64 = 7;

/// An error as libpq reports it: the `ExecStatusType` of the failed
/// result, the `SQLSTATE` (none for an error of libpq itself) and
/// `PQerrorMessage()`.
#[derive(Debug, Clone)]
pub(super) struct PgError {
    pub status: i64,
    pub sql_state: Option<String>,
    pub message: String,
}

impl PgError {
    fn libpq(message: impl Into<String>) -> Self {
        Self {
            status: PGRES_FATAL_ERROR,
            sql_state: None,
            message: message.into(),
        }
    }
}

/// libpq's message for an I/O error: what `strerror()` says.
fn strerror(error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::ConnectionRefused => "Connection refused".to_owned(),
        io::ErrorKind::NotFound => "No such file or directory".to_owned(),
        io::ErrorKind::PermissionDenied => "Permission denied".to_owned(),
        io::ErrorKind::TimedOut => "Connection timed out".to_owned(),
        io::ErrorKind::ConnectionReset => "Connection reset by peer".to_owned(),
        _ => {
            let text = error.to_string();
            match text.find(" (os error") {
                Some(end) => text[..end].to_owned(),
                None => text,
            }
        }
    }
}

const SERVER_CLOSED: &str = "server closed the connection unexpectedly\n\tThis probably means the server terminated abnormally\n\tbefore or while processing the request.\n";

/// An error while reading from or writing to the server.
fn io_error(error: &io::Error, receiving: bool) -> PgError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        return PgError::libpq(SERVER_CLOSED);
    }
    let what = if receiving {
        "could not receive data from server"
    } else {
        "could not send data to server"
    };
    PgError::libpq(format!("{what}: {}\n", strerror(error)))
}

/// The connection parameters `pdo_pgsql` gives `PQconnectdb()`.
#[derive(Debug, Clone, Default)]
pub(super) struct ConnInfo {
    pub host: Option<String>,
    pub port: Option<String>,
    pub dbname: Option<String>,
    pub user: String,
    pub password: String,
    pub sslmode: Option<String>,
    pub application_name: Option<String>,
    /// `connect_timeout` (PDO's `ATTR_TIMEOUT`, 30 by default).
    pub connect_timeout: u64,
}

/// A stream to the server.
trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

/// A result column: `PQfname()` and `PQftype()`.
#[derive(Debug, Clone)]
pub(super) struct Field {
    pub name: Vec<u8>,
    pub type_oid: u32,
}

/// A successful result.
#[derive(Debug, Clone, Default)]
pub(super) struct PgResult {
    /// `PGRES_TUPLES_OK` (rows) rather than `PGRES_COMMAND_OK`.
    pub tuples: bool,
    pub fields: Vec<Field>,
    pub rows: Vec<Vec<Option<Vec<u8>>>>,
    /// The command tag (`INSERT 0 1`, `UPDATE 3`, ...).
    pub command_tag: String,
}

impl PgResult {
    /// `PQcmdTuples()`: the row count of the command tag, or an empty
    /// string.
    pub fn cmd_tuples(&self) -> &str {
        let tag = self.command_tag.as_str();
        let count = if let Some(rest) = tag.strip_prefix("INSERT ") {
            rest.split_once(' ').map(|(_, rows)| rows)
        } else {
            [
                "DELETE ", "UPDATE ", "SELECT ", "MOVE ", "FETCH ", "COPY ", "MERGE ",
            ]
            .iter()
            .find_map(|command| tag.strip_prefix(command))
        };
        count
            .filter(|count| !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()))
            .unwrap_or("")
    }
}

/// A parameter of `PQexecParams()`: `None` is SQL NULL; the bytes are
/// sent as text (`binary` false) or as bytes.
#[derive(Debug, Clone)]
pub(super) struct Param {
    pub value: Option<Vec<u8>>,
    pub binary: bool,
}

/// A connection to the server (libpq's `PGconn`).
pub(super) struct PgConn {
    stream: Box<dyn Stream>,
    buffer: BytesMut,
    parameters: Vec<(String, String)>,
    /// The status of the last `ReadyForQuery` (`I`, `T` or `E`).
    transaction_status: u8,
    /// The connection is lost.
    bad: bool,
}

impl std::fmt::Debug for PgConn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PgConn")
            .field("parameters", &self.parameters)
            .field("transaction_status", &self.transaction_status)
            .finish_non_exhaustive()
    }
}

/// `user`: the operating system user when it is empty (libpq's
/// `pg_fe_getauthname()`).
fn default_user() -> String {
    std::env::var("PGUSER")
        .or_else(|_| std::env::var("USER"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}

/// A field of an error or notice.
fn error_fields(body: &ErrorResponseBody) -> Vec<(u8, String)> {
    let mut fields = Vec::new();
    let mut iterator = body.fields();
    while let Ok(Some(field)) = iterator.next() {
        fields.push((
            field.type_(),
            String::from_utf8_lossy(field.value_bytes()).into_owned(),
        ));
    }
    fields
}

fn field(fields: &[(u8, String)], code: u8) -> Option<&str> {
    fields
        .iter()
        .find(|(type_, _)| *type_ == code)
        .map(|(_, value)| value.as_str())
}

/// The screen width of a character (`pg_encoding_dsplen()` in UTF-8):
/// 2 for the East Asian wide ranges, else 1.
fn display_width(c: char) -> usize {
    let c = u32::from(c);
    let wide = (0x1100..=0x115f).contains(&c)
        || (0x2e80..=0xa4cf).contains(&c) && c != 0x303f
        || (0xac00..=0xd7a3).contains(&c)
        || (0xf900..=0xfaff).contains(&c)
        || (0xfe30..=0xfe6f).contains(&c)
        || (0xff00..=0xff5f).contains(&c)
        || (0xffe0..=0xffe6).contains(&c)
        || (0x20000..=0x2fffd).contains(&c)
        || (0x30000..=0x3fffd).contains(&c);
    if wide { 2 } else { 1 }
}

/// libpq's `reportErrorPosition()`: the line of `query` that holds the
/// 1-based character position `loc`, cut to about 60 screen columns, and
/// a caret under the position.
fn report_error_position(message: &mut String, query: &str, loc: usize) {
    const DISPLAY_SIZE: usize = 60;
    const MIN_RIGHT_CUT: usize = 10;

    let Some(loc) = loc.checked_sub(1) else {
        return;
    };

    // Tabs become spaces.
    let chars: Vec<char> = query
        .chars()
        .map(|c| if c == '\t' { ' ' } else { c })
        .collect();
    let mut scridx: Vec<usize> = Vec::with_capacity(chars.len() + 1);
    let mut scroffset = 0;
    let mut loc_line = 1;
    let mut ibeg = 0;
    let mut iend = None;

    let mut cno = 0;
    while cno < chars.len() {
        let ch = chars[cno];
        scridx.push(scroffset);
        if ch == '\r' || ch == '\n' {
            if cno < loc {
                if ch == '\r' || cno == 0 || chars[cno - 1] != '\r' {
                    loc_line += 1;
                }
                ibeg = cno + 1;
            } else {
                iend = Some(cno);
                break;
            }
        }
        scroffset += display_width(ch);
        cno += 1;
    }
    let mut iend = iend.unwrap_or_else(|| {
        scridx.push(scroffset);
        cno
    });

    if loc > cno {
        return;
    }

    let mut beg_trunc = false;
    let mut end_trunc = false;
    if scridx[iend] - scridx[ibeg] > DISPLAY_SIZE {
        if scridx[ibeg] + DISPLAY_SIZE >= scridx[loc] + MIN_RIGHT_CUT {
            while scridx[iend] - scridx[ibeg] > DISPLAY_SIZE {
                iend -= 1;
            }
            end_trunc = true;
        } else {
            while scridx[loc] + MIN_RIGHT_CUT < scridx[iend] {
                iend -= 1;
                end_trunc = true;
            }
            while scridx[iend] - scridx[ibeg] > DISPLAY_SIZE {
                ibeg += 1;
                beg_trunc = true;
            }
        }
    }

    let mut prefix = format!("LINE {loc_line}: ");
    if beg_trunc {
        prefix.push_str("...");
    }
    let prefix_width: usize = prefix.chars().map(display_width).sum();
    message.push_str(&prefix);
    message.extend(&chars[ibeg..iend]);
    if end_trunc {
        message.push_str("...");
    }
    message.push('\n');
    let caret = prefix_width + scridx[loc] - scridx[ibeg];
    message.push_str(&" ".repeat(caret));
    message.push_str("^\n");
}

/// `pqBuildErrorMessage3()` with the default verbosity and context
/// display: the message of an error the server sent for `query`.
fn build_error_message(fields: &[(u8, String)], query: Option<&str>) -> String {
    let mut message = String::new();
    if let Some(severity) = field(fields, b'S') {
        let _ = write!(message, "{severity}:  ");
    }
    if let Some(primary) = field(fields, b'M') {
        message.push_str(primary);
    }
    let mut cursor: Option<(String, usize)> = None;
    if let Some(position) = field(fields, b'P') {
        if let Some(query) = query {
            cursor = Some((query.to_owned(), position.parse().unwrap_or(0)));
        } else {
            let _ = write!(message, " at character {position}");
        }
    } else if let Some(position) = field(fields, b'p') {
        if let Some(internal_query) = field(fields, b'q') {
            cursor = Some((internal_query.to_owned(), position.parse().unwrap_or(0)));
        } else {
            let _ = write!(message, " at character {position}");
        }
    }
    message.push('\n');
    if let Some((query, position)) = cursor
        && position > 0
    {
        report_error_position(&mut message, &query, position);
    }
    if let Some(detail) = field(fields, b'D') {
        let _ = writeln!(message, "DETAIL:  {detail}");
    }
    if let Some(hint) = field(fields, b'H') {
        let _ = writeln!(message, "HINT:  {hint}");
    }
    if let Some(internal_query) = field(fields, b'q') {
        let _ = writeln!(message, "QUERY:  {internal_query}");
    }
    if let Some(context) = field(fields, b'W') {
        let _ = writeln!(message, "CONTEXT:  {context}");
    }
    message
}

/// The error of an `ErrorResponse` to `query`.
fn server_error(body: &ErrorResponseBody, query: Option<&str>) -> PgError {
    let fields = error_fields(body);
    PgError {
        status: PGRES_FATAL_ERROR,
        sql_state: field(&fields, b'C').map(str::to_owned),
        message: build_error_message(&fields, query),
    }
}

fn protocol_error(error: &io::Error) -> PgError {
    PgError::libpq(format!("invalid message format: {error}\n"))
}

fn columns(body: &RowDescriptionBody) -> Result<Vec<Field>, PgError> {
    let mut fields = Vec::new();
    let mut iterator = body.fields();
    while let Some(field) = iterator.next().map_err(|e| protocol_error(&e))? {
        fields.push(Field {
            name: field.name().as_bytes().to_vec(),
            type_oid: field.type_oid(),
        });
    }
    Ok(fields)
}

impl PgConn {
    /// `PQconnectdb()`: on failure, the error has libpq's message.
    pub async fn connect(info: &ConnInfo) -> Result<Self, PgError> {
        if let Some(mode) = &info.sslmode
            && !matches!(mode.as_str(), "disable" | "allow" | "prefer")
        {
            return Err(PgError::libpq(format!(
                "sslmode value \"{mode}\" needs TLS, which this driver does not support\n"
            )));
        }

        let user = if info.user.is_empty() {
            default_user()
        } else {
            info.user.clone()
        };
        let dbname = info
            .dbname
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| user.clone());
        let host = info
            .host
            .clone()
            .filter(|host| !host.is_empty())
            .or_else(|| std::env::var("PGHOST").ok().filter(|h| !h.is_empty()));
        let port = info
            .port
            .clone()
            .filter(|port| !port.is_empty())
            .or_else(|| std::env::var("PGPORT").ok().filter(|p| !p.is_empty()))
            .unwrap_or_else(|| "5432".to_owned());
        let Ok(port_number) = port.parse::<u16>() else {
            return Err(PgError::libpq(format!(
                "invalid integer value \"{port}\" for connection option \"port\"\n"
            )));
        };
        let timeout = Duration::from_secs(info.connect_timeout.max(2));

        let mut errors = String::new();
        for (target, stream) in open_streams(host.as_deref(), &port, port_number, timeout).await? {
            let stream = match stream {
                Ok(stream) => stream,
                Err(message) => {
                    let _ = write!(errors, "{target} failed: {message}");
                    continue;
                }
            };
            let mut connection = Self {
                stream,
                buffer: BytesMut::with_capacity(8192),
                parameters: Vec::new(),
                transaction_status: b'I',
                bad: false,
            };
            let startup =
                tokio::time::timeout(timeout, connection.startup(&user, &dbname, info)).await;
            match startup {
                Ok(Ok(())) => return Ok(connection),
                Ok(Err(error)) => {
                    let _ = write!(errors, "{target} failed: {}", error.message);
                }
                Err(_) => {
                    let _ = writeln!(errors, "{target} failed: timeout expired");
                }
            }
        }
        Err(PgError::libpq(errors))
    }

    async fn send(&mut self, buffer: &[u8]) -> Result<(), PgError> {
        let written = self.stream.write_all(buffer).await;
        let flushed = match written {
            Ok(()) => self.stream.flush().await,
            Err(e) => Err(e),
        };
        flushed.map_err(|e| {
            self.bad = true;
            io_error(&e, false)
        })
    }

    async fn read_message(&mut self) -> Result<Message, PgError> {
        loop {
            match Message::parse(&mut self.buffer) {
                Ok(Some(message)) => return Ok(message),
                Ok(None) => {}
                Err(e) => {
                    self.bad = true;
                    return Err(protocol_error(&e));
                }
            }
            match self.stream.read_buf(&mut self.buffer).await {
                Ok(0) => {
                    self.bad = true;
                    return Err(PgError::libpq(SERVER_CLOSED));
                }
                Ok(_) => {}
                Err(e) => {
                    self.bad = true;
                    return Err(io_error(&e, true));
                }
            }
        }
    }

    async fn startup(&mut self, user: &str, dbname: &str, info: &ConnInfo) -> Result<(), PgError> {
        let mut parameters = vec![("user", user), ("database", dbname)];
        if let Some(name) = &info.application_name {
            parameters.push(("application_name", name.as_str()));
        }
        let mut buffer = BytesMut::new();
        frontend::startup_message(parameters, &mut buffer).map_err(|e| protocol_error(&e))?;
        self.send(&buffer).await?;

        let mut scram: Option<ScramSha256> = None;
        loop {
            match self.read_message().await? {
                Message::AuthenticationOk
                | Message::BackendKeyData(_)
                | Message::NoticeResponse(_) => {}
                Message::AuthenticationCleartextPassword => {
                    let password = Self::password(info)?;
                    let mut buffer = BytesMut::new();
                    frontend::password_message(password.as_bytes(), &mut buffer)
                        .map_err(|e| protocol_error(&e))?;
                    self.send(&buffer).await?;
                }
                Message::AuthenticationMd5Password(body) => {
                    let password = Self::password(info)?;
                    let hash = md5_hash(user.as_bytes(), password.as_bytes(), body.salt());
                    let mut buffer = BytesMut::new();
                    frontend::password_message(hash.as_bytes(), &mut buffer)
                        .map_err(|e| protocol_error(&e))?;
                    self.send(&buffer).await?;
                }
                Message::AuthenticationSasl(body) => {
                    let mut mechanisms = body.mechanisms();
                    let mut supported = false;
                    while let Some(mechanism) = mechanisms.next().map_err(|e| protocol_error(&e))? {
                        supported |= mechanism == SCRAM_SHA_256;
                    }
                    if !supported {
                        return Err(PgError::libpq(
                            "none of the server's SASL authentication mechanisms are supported\n",
                        ));
                    }
                    let password = Self::password(info)?;
                    let state =
                        ScramSha256::new(password.as_bytes(), ChannelBinding::unsupported());
                    let mut buffer = BytesMut::new();
                    frontend::sasl_initial_response(SCRAM_SHA_256, state.message(), &mut buffer)
                        .map_err(|e| protocol_error(&e))?;
                    self.send(&buffer).await?;
                    scram = Some(state);
                }
                Message::AuthenticationSaslContinue(body) => {
                    let Some(state) = scram.as_mut() else {
                        return Err(PgError::libpq("unexpected SASL message\n"));
                    };
                    state
                        .update(body.data())
                        .map_err(|e| PgError::libpq(format!("{e}\n")))?;
                    let mut buffer = BytesMut::new();
                    frontend::sasl_response(state.message(), &mut buffer)
                        .map_err(|e| protocol_error(&e))?;
                    self.send(&buffer).await?;
                }
                Message::AuthenticationSaslFinal(body) => {
                    let Some(state) = scram.as_mut() else {
                        return Err(PgError::libpq("unexpected SASL message\n"));
                    };
                    state
                        .finish(body.data())
                        .map_err(|e| PgError::libpq(format!("{e}\n")))?;
                }
                Message::ParameterStatus(body) => {
                    let name = body.name().map_err(|e| protocol_error(&e))?.to_owned();
                    let value = body.value().map_err(|e| protocol_error(&e))?.to_owned();
                    self.set_parameter(name, value);
                }
                Message::ReadyForQuery(body) => {
                    self.transaction_status = body.status();
                    return Ok(());
                }
                Message::ErrorResponse(body) => return Err(server_error(&body, None)),
                _ => {
                    return Err(PgError::libpq(
                        "authentication method requested by the server is not supported\n",
                    ));
                }
            }
        }
    }

    fn password(info: &ConnInfo) -> Result<String, PgError> {
        if info.password.is_empty() {
            let from_env = std::env::var("PGPASSWORD").unwrap_or_default();
            if from_env.is_empty() {
                return Err(PgError::libpq("fe_sendauth: no password supplied\n"));
            }
            return Ok(from_env);
        }
        Ok(info.password.clone())
    }

    fn set_parameter(&mut self, name: String, value: String) {
        if let Some(entry) = self.parameters.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = value;
        } else {
            self.parameters.push((name, value));
        }
    }

    /// `PQparameterStatus()`.
    pub fn parameter_status(&self, name: &str) -> Option<&str> {
        self.parameters
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// `PQtransactionStatus() > PQTRANS_IDLE`.
    pub fn in_transaction(&self) -> bool {
        !self.bad && self.transaction_status != b'I'
    }

    /// `PQexec()`: the simple query protocol; the result of the last
    /// statement, or the first error.
    pub async fn exec(&mut self, sql: &str) -> Result<PgResult, PgError> {
        if self.bad {
            return Err(PgError::libpq("no connection to the server\n"));
        }
        let mut buffer = BytesMut::new();
        frontend::query(sql, &mut buffer).map_err(|e| protocol_error(&e))?;
        self.send(&buffer).await?;
        self.read_results(sql).await
    }

    /// `PQexecParams()`: the unnamed statement, its parameters of unknown
    /// type, text results.
    pub async fn exec_params(&mut self, sql: &str, params: &[Param]) -> Result<PgResult, PgError> {
        if self.bad {
            return Err(PgError::libpq("no connection to the server\n"));
        }
        let mut buffer = BytesMut::new();
        frontend::parse("", sql, params.iter().map(|_| 0), &mut buffer)
            .map_err(|e| protocol_error(&e))?;
        let bound = frontend::bind(
            "",
            "",
            params.iter().map(|p| i16::from(p.binary)),
            params.iter(),
            |param, buffer| {
                Ok(match &param.value {
                    Some(value) => {
                        buffer.extend_from_slice(value);
                        postgres_protocol::IsNull::No
                    }
                    None => postgres_protocol::IsNull::Yes,
                })
            },
            Some(0),
            &mut buffer,
        );
        if let Err(error) = bound {
            let message = match error {
                frontend::BindError::Conversion(e) => e.to_string(),
                frontend::BindError::Serialization(e) => e.to_string(),
            };
            return Err(PgError::libpq(format!("{message}\n")));
        }
        frontend::describe(b'P', "", &mut buffer).map_err(|e| protocol_error(&e))?;
        frontend::execute("", 0, &mut buffer).map_err(|e| protocol_error(&e))?;
        frontend::sync(&mut buffer);
        self.send(&buffer).await?;
        self.read_results(sql).await
    }

    /// Reads up to `ReadyForQuery`: the last result, or the first error.
    async fn read_results(&mut self, query: &str) -> Result<PgResult, PgError> {
        let mut error: Option<PgError> = None;
        let mut last: Option<Result<PgResult, PgError>> = None;
        let mut current = PgResult::default();
        loop {
            let message = match self.read_message().await {
                Ok(message) => message,
                Err(e) => return Err(error.unwrap_or(e)),
            };
            match message {
                Message::ParseComplete
                | Message::BindComplete
                | Message::NoData
                | Message::NoticeResponse(_)
                | Message::ParameterDescription(_)
                | Message::BackendKeyData(_)
                | Message::NotificationResponse(_)
                | Message::PortalSuspended
                | Message::CopyData(_)
                | Message::CopyDone
                | Message::CloseComplete => {}
                Message::ParameterStatus(body) => {
                    if let (Ok(name), Ok(value)) = (body.name(), body.value()) {
                        self.set_parameter(name.to_owned(), value.to_owned());
                    }
                }
                Message::RowDescription(body) => {
                    current.fields = columns(&body)?;
                    current.tuples = true;
                }
                Message::DataRow(body) => {
                    let mut row = Vec::with_capacity(current.fields.len());
                    let mut ranges = body.ranges();
                    while let Some(range) = ranges.next().map_err(|e| protocol_error(&e))? {
                        row.push(
                            range.and_then(|range| body.buffer().get(range).map(<[u8]>::to_vec)),
                        );
                    }
                    current.rows.push(row);
                }
                Message::CommandComplete(body) => {
                    body.tag()
                        .map_err(|e| protocol_error(&e))?
                        .clone_into(&mut current.command_tag);
                    last = Some(Ok(std::mem::take(&mut current)));
                }
                Message::EmptyQueryResponse => {
                    current = PgResult::default();
                    last = Some(Err(PgError {
                        status: PGRES_EMPTY_QUERY,
                        sql_state: None,
                        message: String::new(),
                    }));
                }
                Message::ErrorResponse(body) => {
                    let failure = server_error(&body, Some(query));
                    if field(&error_fields(&body), b'V') == Some("FATAL") {
                        self.bad = true;
                    }
                    if error.is_none() {
                        error = Some(failure);
                    }
                    current = PgResult::default();
                }
                Message::ReadyForQuery(body) => {
                    self.transaction_status = body.status();
                    if let Some(error) = error {
                        return Err(error);
                    }
                    return last.unwrap_or_else(|| {
                        Err(PgError {
                            status: PGRES_EMPTY_QUERY,
                            sql_state: None,
                            message: String::new(),
                        })
                    });
                }
                Message::CopyInResponse(_) | Message::CopyOutResponse(_) => {
                    self.bad = true;
                    return Err(PgError::libpq("COPY is not supported by this driver\n"));
                }
                _ => {
                    self.bad = true;
                    return Err(PgError::libpq("unexpected message from the server\n"));
                }
            }
        }
    }

    /// `PQescapeStringConn()` inside single quotes: the value up to its
    /// first NUL byte, quotes doubled (and backslashes when
    /// `standard_conforming_strings` is off).
    pub fn quote(&self, value: &[u8]) -> Vec<u8> {
        let double_backslashes =
            self.parameter_status("standard_conforming_strings") == Some("off");
        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
        let mut quoted = vec![b'\''];
        for &byte in &value[..end] {
            if byte == b'\'' || (double_backslashes && byte == b'\\') {
                quoted.push(byte);
            }
            quoted.push(byte);
        }
        quoted.push(b'\'');
        quoted
    }
}

/// The streams `PQconnectdb()` tries, with the text that names each in
/// its messages (`connection to server at "host" (address), port N`).
async fn open_streams(
    host: Option<&str>,
    port: &str,
    port_number: u16,
    timeout: Duration,
) -> Result<Vec<(String, Result<Box<dyn Stream>, String>)>, PgError> {
    let host = match host {
        Some(host) => host.to_owned(),
        None => default_socket_directory(),
    };

    if host.starts_with('/') {
        let path = format!("{host}/.s.PGSQL.{port}");
        let target = format!("connection to server on socket \"{path}\"");
        return Ok(vec![(target, connect_socket(&path, timeout).await)]);
    }

    let is_address = host.parse::<std::net::IpAddr>().is_ok();
    let addresses: Vec<std::net::SocketAddr> =
        match tokio::net::lookup_host((host.as_str(), port_number)).await {
            Ok(addresses) => addresses.collect(),
            Err(e) => {
                return Err(PgError::libpq(format!(
                    "could not translate host name \"{host}\" to address: {}\n",
                    strerror(&e)
                )));
            }
        };

    let mut streams = Vec::new();
    for address in addresses {
        let target = if is_address {
            format!("connection to server at \"{host}\", port {port}")
        } else {
            format!(
                "connection to server at \"{host}\" ({}), port {port}",
                address.ip()
            )
        };
        let stream =
            match tokio::time::timeout(timeout, tokio::net::TcpStream::connect(address)).await {
                Ok(Ok(stream)) => {
                    let _ = stream.set_nodelay(true);
                    Ok(Box::new(stream) as Box<dyn Stream>)
                }
                Ok(Err(e)) => Err(format!(
                    "{}\n\tIs the server running on that host and accepting TCP/IP connections?\n",
                    strerror(&e)
                )),
                Err(_) => Err("timeout expired\n".to_owned()),
            };
        streams.push((target, stream));
    }
    Ok(streams)
}

/// libpq's default socket directory (Debian's, as in the PHP image).
fn default_socket_directory() -> String {
    if cfg!(unix) {
        "/var/run/postgresql".to_owned()
    } else {
        "localhost".to_owned()
    }
}

#[cfg(unix)]
async fn connect_socket(path: &str, timeout: Duration) -> Result<Box<dyn Stream>, String> {
    match tokio::time::timeout(timeout, tokio::net::UnixStream::connect(path)).await {
        Ok(Ok(stream)) => Ok(Box::new(stream)),
        Ok(Err(e)) => Err(format!(
            "{}\n\tIs the server running locally and accepting connections on that socket?\n",
            strerror(&e)
        )),
        Err(_) => Err("timeout expired\n".to_owned()),
    }
}

#[cfg(not(unix))]
#[allow(clippy::unused_async, reason = "matches the async Unix implementation")]
async fn connect_socket(_path: &str, _timeout: Duration) -> Result<Box<dyn Stream>, String> {
    Err("Unix-domain sockets are not supported on this platform\n".to_owned())
}

/// `PQunescapeBytea()`: the bytes of a `bytea` text, in the hex format
/// (`\x...`) or the escape format (`\\` and `\nnn` octal escapes).
pub(super) fn unescape_bytea(text: &[u8]) -> Vec<u8> {
    if let Some(hex) = text.strip_prefix(b"\\x") {
        let digit = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        };
        let mut bytes = Vec::with_capacity(hex.len() / 2);
        let mut high: Option<u8> = None;
        for &b in hex {
            // Bad characters are skipped, as libpq does.
            let Some(value) = digit(b) else { continue };
            match high.take() {
                Some(h) => bytes.push((h << 4) | value),
                None => high = Some(value),
            }
        }
        return bytes;
    }

    let mut bytes = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i] == b'\\' {
            if text.get(i + 1) == Some(&b'\\') {
                bytes.push(b'\\');
                i += 2;
                continue;
            }
            let octal = text.get(i + 1..i + 4).filter(|digits| {
                digits[0] >= b'0'
                    && digits[0] <= b'3'
                    && (b'0'..=b'7').contains(&digits[1])
                    && (b'0'..=b'7').contains(&digits[2])
            });
            if let Some(digits) = octal {
                bytes.push(
                    ((digits[0] - b'0') << 6) | ((digits[1] - b'0') << 3) | (digits[2] - b'0'),
                );
                i += 4;
                continue;
            }
        }
        bytes.push(text[i]);
        i += 1;
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_position_is_shown_under_the_line() {
        let fields = vec![
            (b'S', "ERROR".to_owned()),
            (b'C', "42601".to_owned()),
            (b'M', "syntax error at or near \"SELEC\"".to_owned()),
            (b'P', "1".to_owned()),
        ];
        assert_eq!(
            build_error_message(&fields, Some("SELEC 1")),
            "ERROR:  syntax error at or near \"SELEC\"\nLINE 1: SELEC 1\n        ^\n"
        );
    }

    #[test]
    fn error_position_on_a_later_line() {
        let mut message = String::new();
        report_error_position(&mut message, "SELECT 1,\n  FROM t", 13);
        assert_eq!(message, "LINE 2:   FROM t\n          ^\n");
    }

    #[test]
    fn detail_and_hint_follow() {
        let fields = vec![
            (b'S', "ERROR".to_owned()),
            (
                b'M',
                "duplicate key value violates unique constraint \"t_pkey\"".to_owned(),
            ),
            (b'D', "Key (id)=(1) already exists.".to_owned()),
        ];
        assert_eq!(
            build_error_message(&fields, Some("INSERT INTO t VALUES (1)")),
            "ERROR:  duplicate key value violates unique constraint \"t_pkey\"\nDETAIL:  Key (id)=(1) already exists.\n"
        );
    }

    #[test]
    fn command_tuples() {
        let result = |tag: &str| PgResult {
            command_tag: tag.to_owned(),
            ..PgResult::default()
        };
        assert_eq!(result("INSERT 0 3").cmd_tuples(), "3");
        assert_eq!(result("UPDATE 2").cmd_tuples(), "2");
        assert_eq!(result("CREATE TABLE").cmd_tuples(), "");
    }

    #[test]
    fn bytea_formats() {
        assert_eq!(unescape_bytea(b"\\x00ff41"), vec![0, 255, b'A']);
        assert_eq!(unescape_bytea(b"a\\\\b\\001"), b"a\\b\x01".to_vec());
    }

    /// A backend message: its tag and body.
    fn backend(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut message = vec![tag];
        let length = u32::try_from(body.len() + 4).unwrap();
        message.extend_from_slice(&length.to_be_bytes());
        message.extend_from_slice(body);
        message
    }

    fn cstr(s: &str) -> Vec<u8> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        bytes
    }

    /// Reads one frontend message (tag and body) from the mock's socket.
    async fn frontend_message(socket: &mut tokio::net::TcpStream) -> (u8, Vec<u8>) {
        let tag = socket.read_u8().await.unwrap();
        let length = socket.read_u32().await.unwrap();
        let mut body = vec![0; usize::try_from(length - 4).unwrap()];
        socket.read_exact(&mut body).await.unwrap();
        (tag, body)
    }

    /// A mock backend: MD5 authentication, then one extended-protocol
    /// query answered with a row, then a simple query answered with an
    /// error.
    #[tokio::test]
    #[allow(
        clippy::too_many_lines,
        reason = "the mock's script and the client's calls"
    )]
    async fn speaks_the_protocol_with_a_mock_backend() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let backend = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            // The startup message has no tag.
            let length = socket.read_u32().await.unwrap();
            let mut startup = vec![0; usize::try_from(length - 4).unwrap()];
            socket.read_exact(&mut startup).await.unwrap();
            assert!(startup.windows(4).any(|w| w == b"user"));

            let mut md5 = 5u32.to_be_bytes().to_vec();
            md5.extend_from_slice(&[1, 2, 3, 4]);
            socket.write_all(&backend(b'R', &md5)).await.unwrap();
            let (tag, body) = frontend_message(&mut socket).await;
            assert_eq!(tag, b'p');
            assert_eq!(body, cstr(&md5_hash(b"u", b"secret", [1, 2, 3, 4])));

            let mut reply = backend(b'R', &0u32.to_be_bytes());
            let mut status = cstr("server_version");
            status.extend(cstr("17.2 (Debian 17.2-1.pgdg120+1)"));
            reply.extend(backend(b'S', &status));
            reply.extend(backend(b'Z', b"I"));
            socket.write_all(&reply).await.unwrap();

            // Parse, Bind, Describe, Execute, Sync.
            let mut tags = Vec::new();
            loop {
                let (tag, _) = frontend_message(&mut socket).await;
                tags.push(tag);
                if tag == b'S' {
                    break;
                }
            }
            assert_eq!(tags, b"PBDES");
            let mut reply = backend(b'1', b"");
            reply.extend(backend(b'2', b""));
            let mut description = 2u16.to_be_bytes().to_vec();
            for (name, oid) in [("id", 23u32), ("name", 25u32)] {
                description.extend(cstr(name));
                description.extend_from_slice(&0u32.to_be_bytes());
                description.extend_from_slice(&0u16.to_be_bytes());
                description.extend_from_slice(&oid.to_be_bytes());
                description.extend_from_slice(&4i16.to_be_bytes());
                description.extend_from_slice(&(-1i32).to_be_bytes());
                description.extend_from_slice(&0i16.to_be_bytes());
            }
            reply.extend(backend(b'T', &description));
            let mut row = 2u16.to_be_bytes().to_vec();
            row.extend_from_slice(&1i32.to_be_bytes());
            row.extend_from_slice(b"7");
            row.extend_from_slice(&(-1i32).to_be_bytes());
            reply.extend(backend(b'D', &row));
            reply.extend(backend(b'C', &cstr("SELECT 1")));
            reply.extend(backend(b'Z', b"T"));
            socket.write_all(&reply).await.unwrap();

            let (tag, body) = frontend_message(&mut socket).await;
            assert_eq!(tag, b'Q');
            assert_eq!(body, cstr("SELEC 1"));
            let mut error = Vec::new();
            for (code, value) in [
                (b'S', "ERROR"),
                (b'V', "ERROR"),
                (b'C', "42601"),
                (b'M', "syntax error at or near \"SELEC\""),
                (b'P', "1"),
            ] {
                error.push(code);
                error.extend(cstr(value));
            }
            error.push(0);
            let mut reply = backend(b'E', &error);
            reply.extend(backend(b'Z', b"E"));
            socket.write_all(&reply).await.unwrap();
        });

        let info = ConnInfo {
            host: Some("127.0.0.1".to_owned()),
            port: Some(port.to_string()),
            dbname: Some("db".to_owned()),
            user: "u".to_owned(),
            password: "secret".to_owned(),
            connect_timeout: 5,
            ..ConnInfo::default()
        };
        let mut connection = PgConn::connect(&info).await.unwrap();
        assert_eq!(
            connection.parameter_status("server_version"),
            Some("17.2 (Debian 17.2-1.pgdg120+1)")
        );
        assert!(!connection.in_transaction());

        let result = connection
            .exec_params(
                "SELECT $1",
                &[Param {
                    value: Some(b"7".to_vec()),
                    binary: false,
                }],
            )
            .await
            .unwrap();
        assert!(result.tuples);
        assert_eq!(result.fields.len(), 2);
        assert_eq!(result.fields[1].type_oid, 25);
        assert_eq!(result.rows, vec![vec![Some(b"7".to_vec()), None]]);
        assert_eq!(result.cmd_tuples(), "1");
        assert!(connection.in_transaction());

        let error = connection.exec("SELEC 1").await.unwrap_err();
        assert_eq!(error.sql_state.as_deref(), Some("42601"));
        assert_eq!(
            error.message,
            "ERROR:  syntax error at or near \"SELEC\"\nLINE 1: SELEC 1\n        ^\n"
        );
        backend.await.unwrap();
    }

    #[tokio::test]
    async fn a_refused_connection_names_the_server() {
        // A port nothing listens on: bind one and drop it.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let info = ConnInfo {
            host: Some("127.0.0.1".to_owned()),
            port: Some(port.to_string()),
            user: "u".to_owned(),
            connect_timeout: 5,
            ..ConnInfo::default()
        };
        let error = PgConn::connect(&info).await.unwrap_err();
        assert_eq!(
            error.message,
            format!(
                "connection to server at \"127.0.0.1\", port {port} failed: Connection refused\n\tIs the server running on that host and accepting TCP/IP connections?\n"
            )
        );
    }
}
