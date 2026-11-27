//! Port of `Doctrine\DBAL\Tools\Console\Command\RunSqlCommand`: executes
//! arbitrary SQL that can come from a file or directly from the command
//! line.
//!
//! A DBAL exception leaves the command as a `ConsoleError::Other` of the
//! same class and message; the connection's async calls run on the current
//! Tokio runtime (a multi-threaded one) or on a new current-thread runtime.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Tools/Console/Command/RunSqlCommand.php

use std::future::Future;
use std::sync::Arc;

use php_value::{Key, PhpArray, Value};
use symfony_console::command::command::{Command, CommandHandler};
use symfony_console::exception::{ConsoleError, Result};
use symfony_console::helper::table::Row;
use symfony_console::helper::table_cell::Cell;
use symfony_console::input::SuggestedValues;
use symfony_console::input::input::Input;
use symfony_console::input::input_argument::{self, InputArgument};
use symfony_console::input::input_option::{self, InputOption, Shortcut};
use symfony_console::output::output::OutputInterface;
use symfony_console::style::symfony_style::SymfonyStyle;

use crate::tools::console::{ConnectionNotFound, ConnectionProvider};
use crate::{Connection, Exception, ParameterTypes};

/// `RunSqlCommand::class`.
pub const CLASS: &str = "Doctrine\\DBAL\\Tools\\Console\\Command\\RunSqlCommand";

const HELP: &str = r#"The <info>%command.name%</info> command executes the given SQL query and
outputs the results:

<info>php %command.full_name% "SELECT * FROM users"</info>"#;

/// `RunSqlCommand`.
pub struct RunSqlCommand {
    connection_provider: Arc<dyn ConnectionProvider>,
}

impl RunSqlCommand {
    /// `new RunSqlCommand($connectionProvider)`: the command with this
    /// handler.
    pub fn command(connection_provider: Arc<dyn ConnectionProvider>) -> Result<Command> {
        Command::with_handler(
            Self {
                connection_provider,
            },
            None,
        )
    }

    /// `getConnection($input)`.
    fn get_connection(&self, input: &Input) -> Result<Arc<Connection>> {
        let connection_name = input.get_option("connection")?;

        if !connection_name.is_null() {
            let name = connection_name.to_php_string().unwrap_or_default();
            return self
                .connection_provider
                .get_connection(&name.to_string_lossy())
                .map_err(|e| other(ConnectionNotFound::CLASS, e.message()));
        }

        Ok(self.connection_provider.get_default_connection())
    }
}

impl CommandHandler for RunSqlCommand {
    fn class(&self) -> &str {
        CLASS
    }

    fn configure(&mut self, command: &mut Command) -> Result<()> {
        command
            .set_name("dbal:run-sql")?
            .set_description("Executes arbitrary SQL directly from the command line.")
            .set_definition_items(vec![
                InputOption::new(
                    "connection",
                    Shortcut::None,
                    Some(input_option::VALUE_REQUIRED),
                    "The named database connection",
                    Value::Null,
                    SuggestedValues::None,
                )?
                .into(),
                InputArgument::new(
                    "sql",
                    Some(input_argument::REQUIRED),
                    "The SQL statement to execute.",
                    Value::Null,
                    SuggestedValues::None,
                )?
                .into(),
                InputOption::new(
                    "depth",
                    Shortcut::None,
                    Some(input_option::VALUE_REQUIRED),
                    "Dumping depth of result set (deprecated).",
                    Value::Null,
                    SuggestedValues::None,
                )?
                .into(),
                InputOption::new(
                    "force-fetch",
                    Shortcut::None,
                    Some(input_option::VALUE_NONE),
                    "Forces fetching the result.",
                    Value::Null,
                    SuggestedValues::None,
                )?
                .into(),
            ])?
            .set_help(HELP);
        Ok(())
    }

    fn execute(
        &mut self,
        _command: &mut Command,
        input: &mut Input,
        output: &mut dyn OutputInterface,
    ) -> Result<i64> {
        let conn = self.get_connection(input)?;
        let mut io = SymfonyStyle::new(input, output)?;

        let sql = input.get_argument("sql")?;

        if sql.is_null() {
            return Err(other(
                "RuntimeException",
                "Argument \"sql\" is required in order to execute this command correctly.",
            ));
        }
        let sql = sql
            .to_php_string()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        if !input.get_option("depth")?.is_null() {
            io.warning(&["Parameter \"depth\" is deprecated and has no effect anymore."])?;
        }

        let force_fetch = input.get_option("force-fetch")?.is_truthy();

        if starts_with_select(&sql) || force_fetch {
            run_query(&mut io, &conn, &sql)?;
        } else {
            run_statement(&mut io, &conn, &sql)?;
        }

        Ok(0)
    }
}

/// `stripos($sql, 'select') === 0`.
fn starts_with_select(sql: &str) -> bool {
    sql.as_bytes()
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"select"))
}

/// `runQuery($io, $conn, $sql)`.
fn run_query(io: &mut SymfonyStyle<'_>, conn: &Connection, sql: &str) -> Result<()> {
    let result_set =
        block_on(conn.fetch_all_associative(sql, &PhpArray::new(), &ParameterTypes::new()))?
            .map_err(|e| thrown(&e))?;
    let Some(first) = result_set.first() else {
        io.success(&["The query yielded an empty result set."])?;

        return Ok(());
    };

    let headers = first
        .iter()
        .map(|(key, _)| Cell::Value(key_value(key)))
        .collect();
    let rows = result_set
        .iter()
        .map(|row| Row::cells(row.iter().map(|(_, value)| Cell::Value(value.clone()))))
        .collect();
    io.table(headers, rows)
}

/// `runStatement($io, $conn, $sql)`.
fn run_statement(io: &mut SymfonyStyle<'_>, conn: &Connection, sql: &str) -> Result<()> {
    let affected = block_on(conn.execute_statement(sql, &PhpArray::new(), &ParameterTypes::new()))?
        .map_err(|e| thrown(&e))?;
    io.success(&[&format!("{affected} rows affected.")])
}

/// `array_keys()`'s value of a key.
fn key_value(key: &Key) -> Value {
    match key {
        Key::Int(i) => Value::Int(*i),
        Key::Str(s) => Value::String(s.clone()),
    }
}

/// A DBAL exception leaving the command.
fn thrown(e: &Exception) -> ConsoleError {
    other(e.class(), &e.message().to_string_lossy())
}

/// `new $class($message)` leaving the command.
fn other(class: &str, message: &str) -> ConsoleError {
    ConsoleError::Other {
        class: class.to_owned(),
        message: message.to_owned(),
    }
}

/// Runs one of the connection's async calls from the command: on the
/// current Tokio runtime when there is one (it must be a multi-threaded
/// runtime), else on a new current-thread runtime.
fn block_on<F: Future>(future: F) -> Result<F::Output> {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::CurrentThread {
            return Err(other(
                "RuntimeException",
                "A console command cannot wait on a current-thread Tokio runtime; use a multi-threaded one",
            ));
        }
        return Ok(tokio::task::block_in_place(|| handle.block_on(future)));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| other("RuntimeException", &e.to_string()))?;
    Ok(runtime.block_on(future))
}
