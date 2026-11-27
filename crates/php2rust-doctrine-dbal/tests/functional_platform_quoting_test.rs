//! Port of tests/Functional/Platform/QuotingTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879, on `pdo_sqlite` in memory.
//! Names and assertions are PHPUnit's; a data provider's cases run in one
//! test, in order. The `Oracle` skip does not apply on `SQLite`.
//!
//! Upstream: doctrine/dbal tests/Functional/Platform/QuotingTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_manager_support;

use php_value::{Key, PhpArray, Value};
use php2rust_doctrine_dbal::ParameterTypes;
use schema_manager_support::{platform, sqlite};

/// `stringLiteralProvider()`.
const STRING_LITERAL_PROVIDER: [(&str, &str); 2] = [("backslash", "\\"), ("single-quote", "'")];

/// `identifierProvider()`.
const IDENTIFIER_PROVIDER: [&str; 4] = ["[", "]", "\"", "`"];

#[tokio::test]
async fn testQuoteStringLiteral() {
    for (case, string) in STRING_LITERAL_PROVIDER {
        let connection = sqlite();
        let platform = platform(&connection).await;
        let query = platform.get_dummy_select_sql(&platform.quote_string_literal(string));

        let actual = connection
            .fetch_one(&query, &PhpArray::new(), &ParameterTypes::new())
            .await
            .unwrap()
            .unwrap_or(Value::Bool(false));
        assert!(
            Value::string(string).identical(&actual),
            "{case}: {actual:?}"
        );
    }
}

#[tokio::test]
async fn testQuoteIdentifier() {
    for identifier in IDENTIFIER_PROVIDER {
        let connection = sqlite();
        let platform = platform(&connection).await;
        let query = platform.get_dummy_select_sql(&format!(
            "NULL AS {}",
            platform.quote_single_identifier(identifier)
        ));

        let row = connection
            .fetch_associative(&query, &PhpArray::new(), &ParameterTypes::new())
            .await
            .unwrap();

        let row = row.expect("assertNotFalse($row)");
        let key = row.iter().next().map(|(key, _)| key.clone());
        assert_eq!(Some(Key::from(identifier)), key, "{identifier}");
    }
}
