//! Port of tests/Tools/Console/RunSqlCommandTest.php. The connection mock
//! is a real `Connection` on the recording test driver: its queries give
//! the mocked `fetchAllAssociative()` rows, its `exec()` the mocked
//! `executeStatement()` count, and the recorded calls stand for the
//! mock's `once()` and `never()` expectations.
//!
//! Upstream: doctrine/dbal tests/Tools/Console/RunSqlCommandTest.php
#![cfg(feature = "console")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use std::sync::Arc;

use connection_support::{Script, TestDriver, none};
use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::tools::console::SingleConnectionProvider;
use php2rust_doctrine_dbal::tools::console::command::RunSqlCommand;
use symfony_console::application::{Application, SharedCommand};
use symfony_console::tester::command_tester::{CommandTester, ExecuteOptions};

/// tests/Tools/Console/Fixtures/select-1.txt.
const SELECT_1: &str = " --- \n  0  \n --- \n  1  \n --- \n\n";

struct Fixture {
    driver: Arc<TestDriver>,
    command: SharedCommand,
    command_tester: CommandTester,
}

/// `setUp()`, the connection mock answering as `script` says.
fn set_up(script: Script) -> Fixture {
    let driver = TestDriver::new(script);
    let connection = Arc::new(driver.connection(none()));
    let command =
        RunSqlCommand::command(Arc::new(SingleConnectionProvider::new(connection))).unwrap();
    let command = Application::default()
        .add_command(command)
        .unwrap()
        .unwrap();
    let command_tester = CommandTester::new(command.clone());
    Fixture {
        driver,
        command,
        command_tester,
    }
}

impl Fixture {
    /// `['command' => $this->command->getName(), ...$items]`.
    fn input(&self, items: &[(&str, Value)]) -> PhpArray {
        let mut input = PhpArray::new();
        let name = self.command.lock().get_name().unwrap().to_owned();
        input.insert("command", Value::from(name));
        for (key, value) in items {
            input.insert(*key, value.clone());
        }
        input
    }

    fn execute(&mut self, items: &[(&str, Value)]) -> i64 {
        let input = self.input(items);
        self.command_tester
            .execute(input, ExecuteOptions::default())
            .unwrap()
    }

    fn display(&mut self) -> String {
        self.command_tester
            .get_display(false)
            .unwrap()
            .replace("\r\n", "\n")
    }

    fn calls(&self, prefix: &str) -> Vec<String> {
        self.driver
            .calls
            .all()
            .into_iter()
            .filter(|call| call.starts_with(prefix))
            .collect()
    }
}

/// `expectConnectionFetchAllAssociative()`: `fetchAllAssociative()`
/// gives `[[1]]`.
fn fetch_all_associative() -> Script {
    Script {
        rows: vec![vec![("0", Value::Int(1))]],
        ..Script::default()
    }
}

/// `expectConnectionExecuteStatement()`: `executeStatement()` gives 42.
fn execute_statement() -> Script {
    Script {
        affected_rows: 42,
        ..Script::default()
    }
}

#[test]
fn testMissingSqlArgument() {
    let mut fixture = set_up(Script::default());
    let input = fixture.input(&[("sql", Value::Null)]);

    let error = fixture
        .command_tester
        .execute(input, ExecuteOptions::default())
        .unwrap_err();

    assert_eq!(error.class(), "RuntimeException");
    assert_eq!(
        error.message(),
        "Argument \"sql\" is required in order to execute this command correctly."
    );
}

#[test]
fn testSelectStatementsPrintsResult() {
    let mut fixture = set_up(fetch_all_associative());

    let exit_code = fixture.execute(&[("sql", Value::from("SELECT 1"))]);
    assert_eq!(exit_code, 0);

    assert_eq!(fixture.display(), SELECT_1);
    assert_eq!(fixture.calls("query "), ["query SELECT 1"]);
    assert_eq!(fixture.calls("exec "), [] as [std::string::String; 0]);
}

#[test]
fn testSelectWithEmptyResultSet() {
    let mut fixture = set_up(Script::default());

    let exit_code = fixture.execute(&[("sql", Value::from("SELECT foo FROM bar"))]);
    assert_eq!(exit_code, 0);

    assert!(
        fixture
            .display()
            .contains("[OK] The query yielded an empty result set.")
    );
    assert_eq!(fixture.calls("query "), ["query SELECT foo FROM bar"]);
    assert_eq!(fixture.calls("exec "), [] as [std::string::String; 0]);
}

#[test]
fn testUpdateStatementsPrintsAffectedLines() {
    let mut fixture = set_up(execute_statement());

    fixture.execute(&[("sql", Value::from("UPDATE foo SET bar = 42"))]);

    assert!(fixture.display().contains("[OK] 42 rows affected."));
    assert_eq!(fixture.calls("exec "), ["exec UPDATE foo SET bar = 42"]);
    assert_eq!(fixture.calls("query "), [] as [std::string::String; 0]);
}

#[test]
fn testStatementsWithFetchResultPrintsResult() {
    let mut fixture = set_up(fetch_all_associative());

    fixture.execute(&[
        (
            "sql",
            Value::from("\"WITH bar as (SELECT 1) SELECT * FROM bar"),
        ),
        ("--force-fetch", Value::Bool(true)),
    ]);

    assert_eq!(fixture.display(), SELECT_1);
    assert_eq!(fixture.calls("query ").len(), 1);
    assert_eq!(fixture.calls("exec "), [] as [std::string::String; 0]);
}
