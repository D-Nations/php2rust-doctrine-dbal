//! Port of tests/Portability/ResultTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order. The mock
//! is a driver result that records the methods called on it and gives the
//! configured return value; `expects(self::once())->method($source)` is
//! one recorded call of that method.
//!
//! Upstream: doctrine/dbal tests/Portability/ResultTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::driver::Result as DriverResult;
use php2rust_doctrine_dbal::portability::{Converter, Result};

/// What a method of the mock returns.
#[derive(Clone)]
enum Return {
    Row(PhpArray),
    One(Value),
    Rows(Vec<PhpArray>),
    Column(Vec<Value>),
    Count(i64),
    Nothing,
}

/// `createMock(DriverResult::class)` with `->method($source)
/// ->willReturn($return)`.
struct DriverResultMock {
    method: &'static str,
    r#return: Return,
    calls: Arc<Mutex<Vec<&'static str>>>,
}

impl DriverResultMock {
    fn call(&self, method: &'static str) -> Return {
        self.calls.lock().unwrap().push(method);
        assert_eq!(self.method, method, "unexpected call to {method}()");
        self.r#return.clone()
    }
}

impl DriverResult for DriverResultMock {
    fn fetch_numeric(&mut self) -> std::result::Result<Option<PhpArray>, Exception> {
        match self.call("fetchNumeric") {
            Return::Row(row) => Ok(Some(row)),
            _ => Ok(None),
        }
    }

    fn fetch_associative(&mut self) -> std::result::Result<Option<PhpArray>, Exception> {
        match self.call("fetchAssociative") {
            Return::Row(row) => Ok(Some(row)),
            _ => Ok(None),
        }
    }

    fn fetch_one(&mut self) -> std::result::Result<Option<Value>, Exception> {
        match self.call("fetchOne") {
            Return::One(value) => Ok(Some(value)),
            _ => Ok(None),
        }
    }

    fn fetch_all_numeric(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        match self.call("fetchAllNumeric") {
            Return::Rows(rows) => Ok(rows),
            _ => Ok(Vec::new()),
        }
    }

    fn fetch_all_associative(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        match self.call("fetchAllAssociative") {
            Return::Rows(rows) => Ok(rows),
            _ => Ok(Vec::new()),
        }
    }

    fn fetch_first_column(&mut self) -> std::result::Result<Vec<Value>, Exception> {
        match self.call("fetchFirstColumn") {
            Return::Column(values) => Ok(values),
            _ => Ok(Vec::new()),
        }
    }

    fn row_count(&self) -> std::result::Result<i64, Exception> {
        match self.call("rowCount") {
            Return::Count(count) => Ok(count),
            _ => Ok(0),
        }
    }

    fn column_count(&self) -> std::result::Result<usize, Exception> {
        match self.call("columnCount") {
            Return::Count(count) => Ok(usize::try_from(count).unwrap()),
            _ => Ok(0),
        }
    }

    fn free(&mut self) {
        self.call("free");
    }

    fn class(&self) -> &'static str {
        "DriverResultMock"
    }
}

/// `newResult($driverResult)`.
fn new_result(method: &'static str, r#return: Return) -> (Result, Arc<Mutex<Vec<&'static str>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let driver_result = DriverResultMock {
        method,
        r#return,
        calls: Arc::clone(&calls),
    };

    (
        Result::new(
            Box::new(driver_result),
            Arc::new(Converter::new(false, false, None).unwrap()),
        ),
        calls,
    )
}

fn list(values: &[&str]) -> PhpArray {
    PhpArray::list(values.iter().map(|value| Value::string(*value)))
}

fn foo(value: &str) -> PhpArray {
    let mut row = PhpArray::new();
    row.insert("foo", Value::string(value));
    row
}

/// The value a fetch method gave, as a PHP value.
fn value_of(r#return: Return) -> Value {
    match r#return {
        Return::Row(row) => Value::Array(row),
        Return::One(value) => value,
        Return::Rows(rows) => Value::Array(PhpArray::list(rows.into_iter().map(Value::Array))),
        Return::Column(values) => Value::Array(PhpArray::list(values)),
        Return::Count(count) => Value::Int(count),
        Return::Nothing => Value::Null,
    }
}

type Fetch = fn(&mut Result) -> Return;

/// `fetchProvider()`.
fn fetch_provider() -> Vec<(&'static str, &'static str, Fetch, Return)> {
    vec![
        (
            "numeric",
            "fetchNumeric",
            |result| Return::Row(result.fetch_numeric().unwrap().unwrap()),
            Return::Row(list(&["bar"])),
        ),
        (
            "associative",
            "fetchAssociative",
            |result| Return::Row(result.fetch_associative().unwrap().unwrap()),
            Return::Row(foo("bar")),
        ),
        (
            "one",
            "fetchOne",
            |result| Return::One(result.fetch_one().unwrap().unwrap()),
            Return::One(Value::string("bar")),
        ),
        (
            "all-numeric",
            "fetchAllNumeric",
            |result| Return::Rows(result.fetch_all_numeric().unwrap()),
            Return::Rows(vec![list(&["bar"]), list(&["baz"])]),
        ),
        (
            "all-associative",
            "fetchAllAssociative",
            |result| Return::Rows(result.fetch_all_associative().unwrap()),
            Return::Rows(vec![foo("bar"), foo("baz")]),
        ),
        (
            "first-column",
            "fetchFirstColumn",
            |result| Return::Column(result.fetch_first_column().unwrap()),
            Return::Column(vec![Value::string("bar"), Value::string("baz")]),
        ),
    ]
}

#[test]
fn testFetch() {
    for (case, source, fetch, r#return) in fetch_provider() {
        let (mut result, calls) = new_result(source, r#return.clone());

        let fetched = fetch(&mut result);

        assert!(value_of(r#return).identical(&value_of(fetched)), "{case}");
        assert_eq!(vec![source], *calls.lock().unwrap(), "{case}");
    }
}

#[test]
fn testRowCount() {
    let (result, calls) = new_result("rowCount", Return::Count(666));

    assert_eq!(666, result.row_count().unwrap());
    assert_eq!(vec!["rowCount"], *calls.lock().unwrap());
}

#[test]
fn testColumnCount() {
    let (result, calls) = new_result("columnCount", Return::Count(666));

    assert_eq!(666, result.column_count().unwrap());
    assert_eq!(vec!["columnCount"], *calls.lock().unwrap());
}

#[test]
fn testFree() {
    let (mut result, calls) = new_result("free", Return::Nothing);

    result.free();
    assert_eq!(vec!["free"], *calls.lock().unwrap());
}
