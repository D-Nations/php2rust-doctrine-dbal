//! Port of tests/Connection/CachedQueryTest.php.
//!
//! The PHP test mocks the driver connection, whose `query()` returns an
//! `ArrayResult` and is expected a number of times; here the recording
//! `connection_support::TestDriver` returns the `ArrayResult`, and the test
//! counts its `query` calls. The data provider's two `ArrayAdapter`s
//! (serialized and by reference) run in one test each, in order.
//!
//! Upstream: doctrine/dbal tests/Connection/CachedQueryTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use std::sync::Arc;

use connection_support::{Script, TestDriver, no_types, none};
use php_value::{Key, PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::Connection;
use php2rust_doctrine_dbal::cache::{
    ArrayAdapter, ArrayResult, CacheItemPool, PoolValue, QueryCacheProfile,
};

/// `providePsrCacheImplementations()`.
fn psr_cache_implementations() -> [fn() -> Arc<ArrayAdapter>; 2] {
    [
        || Arc::new(ArrayAdapter::with_options(0, true)),
        || Arc::new(ArrayAdapter::with_options(0, false)),
    ]
}

/// `createConnection()`: the driver and the connection; the caller checks
/// the number of queries.
fn create_connection(column_names: &[&str], rows: &[&[&str]]) -> (Arc<TestDriver>, Connection) {
    let result = ArrayResult::new(
        column_names
            .iter()
            .map(|name| PhpString::from(*name))
            .collect(),
        rows.iter()
            .map(|row| PhpArray::list(row.iter().map(|value| Value::string(*value))))
            .collect(),
    );
    let driver = TestDriver::new(Script {
        array_result: Some(result),
        ..Script::default()
    });
    let connection = driver.connection(PhpArray::new());
    (driver, connection)
}

fn row(pairs: &[(&str, &str)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(*key, Value::string(*value));
    }
    array
}

fn profile(key: &str, cache: Arc<ArrayAdapter>) -> QueryCacheProfile {
    let cache: Arc<dyn CacheItemPool> = cache;
    QueryCacheProfile::new(0, Some(key.to_owned()), Some(cache))
}

#[tokio::test]
async fn testCachedQuery() {
    for provider in psr_cache_implementations() {
        let cache = provider();

        let (driver, connection) = create_connection(&["foo"], &[&["bar"]]);
        let qcp = profile("testCachedQuery", cache.clone());

        let mut first_result = connection
            .execute_cache_query("SELECT 1", &none(), &no_types(), &qcp)
            .await
            .unwrap();
        assert_eq!(
            first_result.fetch_all_associative().unwrap(),
            vec![row(&[("foo", "bar")])]
        );
        first_result.free();
        let mut second_result = connection
            .execute_cache_query("SELECT 1", &none(), &no_types(), &qcp)
            .await
            .unwrap();
        assert_eq!(
            second_result.fetch_all_associative().unwrap(),
            vec![row(&[("foo", "bar")])]
        );
        second_result.free();
        assert_eq!(
            connection
                .execute_cache_query("SELECT 1", &none(), &no_types(), &qcp)
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            vec![row(&[("foo", "bar")])]
        );

        let item = cache.get_item("testCachedQuery").unwrap();
        assert_eq!(item.get().and_then(PoolValue::count), Some(1));
        assert_eq!(driver.calls.count("query SELECT 1"), 1);
    }
}

#[tokio::test]
async fn testCachedQueryWithChangedImplementationIsExecutedTwice() {
    for provider in psr_cache_implementations() {
        let (driver, connection) = create_connection(&["baz"], &[&["qux"]]);

        assert_eq!(
            connection
                .execute_cache_query(
                    "SELECT 1",
                    &none(),
                    &no_types(),
                    &profile(
                        "testCachedQueryWithChangedImplementationIsExecutedTwice",
                        provider()
                    ),
                )
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            vec![row(&[("baz", "qux")])]
        );

        assert_eq!(
            connection
                .execute_cache_query(
                    "SELECT 1",
                    &none(),
                    &no_types(),
                    &profile(
                        "testCachedQueryWithChangedImplementationIsExecutedTwice",
                        provider()
                    ),
                )
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            vec![row(&[("baz", "qux")])]
        );
        assert_eq!(driver.calls.count("query SELECT 1"), 2);
    }
}

#[tokio::test]
async fn testOldCacheFormat() {
    for provider in psr_cache_implementations() {
        let (driver, connection) = create_connection(&["foo"], &[&["bar"]]);
        let cache = provider();
        let qcp = profile("testOldCacheFormat", cache.clone());

        let (cache_key, real_key) =
            qcp.generate_cache_keys("SELECT 1", &none(), &no_types(), &PhpArray::new());
        let mut old = PhpArray::new();
        old.insert(
            Key::from(real_key),
            Value::Array(PhpArray::list([Value::Array(row(&[("foo", "bar")]))])),
        );
        cache.save(
            cache
                .get_item(&cache_key)
                .unwrap()
                .set(PoolValue::Value(Value::Array(old))),
        );

        assert_eq!(
            connection
                .execute_cache_query("SELECT 1", &none(), &no_types(), &qcp)
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            vec![row(&[("foo", "bar")])]
        );
        assert_eq!(
            connection
                .execute_cache_query("SELECT 1", &none(), &no_types(), &qcp)
                .await
                .unwrap()
                .fetch_all_associative()
                .unwrap(),
            vec![row(&[("foo", "bar")])]
        );

        let item = cache.get_item("testOldCacheFormat").unwrap();
        assert_eq!(item.get().and_then(PoolValue::count), Some(1));
        assert_eq!(driver.calls.count("query SELECT 1"), 1);
    }
}
