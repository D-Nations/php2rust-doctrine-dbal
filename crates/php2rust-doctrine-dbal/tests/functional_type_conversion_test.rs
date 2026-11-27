//! Port of tests/Functional/TypeConversionTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's; a data provider's rows run in one
//! test, each on a fresh database. `testConversionToUtcDateTime` is not
//! here: it sets the default time zone to Europe/Berlin and reads dates in
//! America/Vancouver, and `php_value` has neither named time zones nor a
//! default one to set (parity keeps it pending).
//!
//! Upstream: doctrine/dbal tests/Functional/TypeConversionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use std::sync::Arc;

use php_value::bcmath::BcNumber;
use php_value::datetime::DateTime;
use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value};
use php2rust_doctrine_dbal::schema::Table;
use php2rust_doctrine_dbal::types::{self, Types};
use php2rust_doctrine_dbal::{Connection, ParameterTypes};
use schema_manager_support::{column, drop_and_create_table, platform, primary_key, sqlite};

/// The connection and `self::$typeCounter`.
struct Test {
    connection: Connection,
    type_counter: i64,
}

async fn set_up() -> Test {
    let connection = sqlite();

    let nullable = |name: &str, type_name: &str| column(name, type_name).set_not_null(false);
    let table = Table::editor()
        .set_unquoted_name("type_conversion", None)
        .set_columns(vec![
            column("id", Types::INTEGER).create().unwrap(),
            nullable("test_string", Types::STRING)
                .set_length(Some(16))
                .create()
                .unwrap(),
            nullable("test_boolean", Types::BOOLEAN).create().unwrap(),
            nullable("test_bigint", Types::BIGINT).create().unwrap(),
            nullable("test_smallint", Types::SMALLINT).create().unwrap(),
            nullable("test_datetime", Types::DATETIME_MUTABLE)
                .create()
                .unwrap(),
            nullable("test_datetime_utc", Types::DATETIME_UTC_MUTABLE)
                .create()
                .unwrap(),
            nullable("test_datetime_utc_immutable", Types::DATETIME_UTC_IMMUTABLE)
                .create()
                .unwrap(),
            nullable("test_datetimetz", Types::DATETIMETZ_MUTABLE)
                .create()
                .unwrap(),
            nullable("test_date", Types::DATE_MUTABLE).create().unwrap(),
            nullable("test_time", Types::TIME_MUTABLE).create().unwrap(),
            nullable("test_text", Types::TEXT).create().unwrap(),
            nullable("test_json", Types::JSON).create().unwrap(),
            nullable("test_float", Types::FLOAT).create().unwrap(),
            nullable("test_smallfloat", Types::SMALLFLOAT)
                .create()
                .unwrap(),
            nullable("test_decimal", Types::DECIMAL)
                .set_precision(Some(10))
                .set_scale(2)
                .create()
                .unwrap(),
            nullable("test_number", Types::NUMBER)
                .set_precision(Some(10))
                .set_scale(2)
                .create()
                .unwrap(),
        ])
        .set_primary_key_constraint(primary_key(&["id"]))
        .create()
        .unwrap();

    drop_and_create_table(&connection, &table).await;

    Test {
        connection,
        type_counter: 0,
    }
}

impl Test {
    /// `processValue($type, $originalValue)`.
    async fn process_value(&mut self, type_: &str, original_value: &Value) -> Value {
        let column_name = format!("test_{type_}");
        let type_instance = types::get_type(type_).unwrap();
        let platform = platform(&self.connection).await;
        let insertion_value = type_instance
            .convert_to_database_value(original_value, platform)
            .unwrap();

        self.type_counter += 1;
        let mut data = PhpArray::new();
        data.insert("id", Value::Int(self.type_counter));
        data.insert(column_name.as_str(), insertion_value);
        self.connection
            .insert("type_conversion", &data, &ParameterTypes::new())
            .await
            .unwrap();

        let sql = format!(
            "SELECT {column_name} FROM type_conversion WHERE id = {}",
            self.type_counter
        );

        let fetched = self
            .connection
            .fetch_one(&sql, &PhpArray::new(), &ParameterTypes::new())
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));

        type_instance
            .convert_to_php_value(&fetched, platform)
            .unwrap()
    }
}

fn assert_equals(expected: &Value, actual: &Value) {
    assert!(
        phpunit_equals(expected, actual),
        "expected {expected:?}, got {actual:?}"
    );
}

/// `new DateTime($text)`.
fn date_time(text: &str) -> Value {
    let date = php_value::date_create::date_create(text.as_bytes(), None)
        .unwrap()
        .unwrap();
    DateTime::object(date)
}

fn mutable_date(value: &Value) -> php_value::datetime::DateTimeImmutable {
    value
        .as_object()
        .and_then(|object| object.downcast_ref::<DateTime>())
        .expect("a DateTime")
        .get()
}

#[tokio::test]
async fn testIdempotentConversionToBoolean() {
    // booleanProvider()
    for (name, type_, original_value) in [
        ("true", Types::BOOLEAN, Value::Bool(true)),
        ("false", Types::BOOLEAN, Value::Bool(false)),
    ] {
        let mut test = set_up().await;
        let db_value = test.process_value(type_, &original_value).await;

        assert!(matches!(db_value, Value::Bool(_)), "{name}: {db_value:?}");
        assert_equals(&original_value, &db_value);
    }
}

#[tokio::test]
async fn testIdempotentConversionToInteger() {
    // integerProvider()
    for (name, type_, original_value) in [("smallint", Types::SMALLINT, Value::Int(123))] {
        let mut test = set_up().await;
        let db_value = test.process_value(type_, &original_value).await;

        assert!(matches!(db_value, Value::Int(_)), "{name}: {db_value:?}");
        assert_equals(&original_value, &db_value);
    }
}

#[tokio::test]
async fn testIdempotentConversionToFloat() {
    // floatProvider()
    for (name, type_, original_value) in [
        ("float", Types::FLOAT, Value::Float(1.5)),
        ("smallfloat", Types::SMALLFLOAT, Value::Float(1.5)),
    ] {
        let mut test = set_up().await;
        let db_value = test.process_value(type_, &original_value).await;

        assert!(matches!(db_value, Value::Float(_)), "{name}: {db_value:?}");
        assert_equals(&original_value, &db_value);
    }
}

#[tokio::test]
async fn testIdempotentConversionToString() {
    // toStringProvider(); pdo_sqlite, not pdo_oci.
    for (name, type_, original_value) in [
        ("string", Types::STRING, Value::string("ABCDEFGabcdefg")),
        ("text", Types::TEXT, Value::string("foo ".repeat(1000))),
    ] {
        let mut test = set_up().await;
        let db_value = test.process_value(type_, &original_value).await;

        assert!(matches!(db_value, Value::String(_)), "{name}: {db_value:?}");
        assert_equals(&original_value, &db_value);
    }
}

#[tokio::test]
async fn testIdempotentConversionToArray() {
    let mut test = set_up().await;
    let mut array = PhpArray::new();
    array.insert("foo", Value::string("bar"));
    let array = Value::Array(array);

    assert_equals(&array, &test.process_value("json", &array).await);
}

#[tokio::test]
async fn testIdempotentConversionToDateTime() {
    // toDateTimeProvider()
    for (name, type_, original_value) in [
        (
            "datetime",
            Types::DATETIME_MUTABLE,
            date_time("2010-04-05 10:10:10"),
        ),
        (
            "datetimetz",
            Types::DATETIMETZ_MUTABLE,
            date_time("2010-04-05 10:10:10"),
        ),
        ("date", Types::DATE_MUTABLE, date_time("2010-04-05")),
        (
            "time",
            Types::TIME_MUTABLE,
            date_time("1970-01-01 10:10:10"),
        ),
    ] {
        let mut test = set_up().await;
        let db_value = test.process_value(type_, &original_value).await;

        assert_eq!(
            db_value.as_object().map(|object| object.class().to_owned()),
            Some("DateTime".to_owned()),
            "{name}"
        );

        if type_ == Types::DATETIMETZ_MUTABLE {
            continue;
        }

        assert_equals(&original_value, &db_value);
        assert_eq!(
            mutable_date(&original_value).timezone(),
            mutable_date(&db_value).timezone(),
            "{name}"
        );
    }
}

#[tokio::test]
async fn testDecimal() {
    let mut test = set_up().await;
    let db_value = test
        .process_value(Types::DECIMAL, &Value::string("13.37"))
        .await;
    assert!(Value::string("13.37").identical(&db_value), "{db_value:?}");
}

/// `#[RequiresPhp('>= 8.4')]`, `#[RequiresPhpExtension('bcmath')]`: the
/// oracle's PHP 8.4.27 runs it.
#[tokio::test]
async fn testNumber() {
    let mut test = set_up().await;
    let original_value = BcNumber::parse(b"13.37").unwrap();
    let db_value = test
        .process_value(
            Types::NUMBER,
            &Value::Object(Arc::new(original_value.clone())),
        )
        .await;

    let db_value = db_value
        .as_object()
        .and_then(|object| object.downcast_ref::<BcNumber>())
        .expect("a BcMath\\Number");
    assert!(original_value.compare(db_value).is_eq());
}
