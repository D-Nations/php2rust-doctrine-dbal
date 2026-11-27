//! Port of tests/Portability/StatementTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's. The mock is a driver statement that records its calls;
//! `expects(self::once())->method(...)->with(...)` is one recorded call
//! with those arguments. Its `execute()` gives an empty result, as a
//! PHPUnit mock's `Result` return value does.
//!
//! Upstream: doctrine/dbal tests/Portability/StatementTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::driver::{
    BoxFuture, Result as DriverResult, Statement as DriverStatement,
};
use php2rust_doctrine_dbal::portability::{Converter, Statement};
use php2rust_doctrine_dbal::{Exception, ParameterType};

/// A recorded call of the mock.
#[derive(Debug, PartialEq)]
enum Call {
    BindValue(Key, Value, ParameterType),
    Execute,
}

/// `createMock(DriverStatement::class)`.
struct DriverStatementMock {
    calls: Arc<Mutex<Vec<Call>>>,
}

impl DriverStatement for DriverStatementMock {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> Result<(), Exception> {
        self.calls
            .lock()
            .unwrap()
            .push(Call::BindValue(param.clone(), value, r#type));
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        self.calls.lock().unwrap().push(Call::Execute);
        Box::pin(async { Ok(Box::new(EmptyResult) as Box<dyn DriverResult>) })
    }
}

/// The `Result` mock a PHPUnit mock returns.
struct EmptyResult;

impl DriverResult for EmptyResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(None)
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(None)
    }

    fn row_count(&self) -> Result<i64, Exception> {
        Ok(0)
    }

    fn column_count(&self) -> Result<usize, Exception> {
        Ok(0)
    }

    fn free(&mut self) {}

    fn class(&self) -> &'static str {
        "ResultMock"
    }
}

/// `setUp()`: the statement under test and the calls of the wrapped one.
fn set_up() -> (Statement, Arc<Mutex<Vec<Call>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let wrapped_stmt = DriverStatementMock {
        calls: Arc::clone(&calls),
    };
    let converter = Converter::new(false, false, None).unwrap();
    let stmt = Statement::new(Box::new(wrapped_stmt), Arc::new(converter));

    (stmt, calls)
}

#[test]
fn testBindValue() {
    let (mut stmt, calls) = set_up();
    let param = Key::from("myparam");
    let value = Value::string("myvalue");
    let r#type = ParameterType::String;

    stmt.bind_value(&param, value.clone(), r#type).unwrap();

    assert_eq!(
        vec![Call::BindValue(param, value, r#type)],
        *calls.lock().unwrap()
    );
}

#[tokio::test]
async fn testExecute() {
    let (mut stmt, calls) = set_up();

    stmt.execute().await.unwrap();

    assert_eq!(vec![Call::Execute], *calls.lock().unwrap());
}
