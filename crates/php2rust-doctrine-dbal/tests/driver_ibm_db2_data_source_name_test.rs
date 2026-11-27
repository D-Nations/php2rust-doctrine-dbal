//! Port of tests/Driver/IBMDB2/DataSourceNameTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order.
//!
//! Upstream: doctrine/dbal tests/Driver/IBMDB2/DataSourceNameTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::driver::ibm_db2::DataSourceName;

fn params(pairs: &[(&str, Value)]) -> PhpArray {
    let mut params = PhpArray::new();
    for (key, value) in pairs {
        params.insert(*key, value.clone());
    }
    params
}

/// `connectionParametersProvider()`.
fn connection_parameters_provider() -> Vec<(&'static str, PhpArray, &'static str)> {
    vec![
        ("empty-params", PhpArray::new(), ""),
        (
            "cataloged-database",
            params(&[
                ("host", Value::string("localhost")),
                ("port", Value::Int(50000)),
                ("dbname", Value::string("doctrine")),
                ("user", Value::string("db2inst1")),
                ("password", Value::string("Passw0rd")),
            ]),
            "HOSTNAME=localhost;PORT=50000;DATABASE=doctrine;UID=db2inst1;PWD=Passw0rd",
        ),
        (
            "uncataloged-database",
            params(&[(
                "dbname",
                Value::string(
                    "HOSTNAME=localhost;PORT=50000;DATABASE=doctrine;UID=db2inst1;PWD=Passw0rd",
                ),
            )]),
            "HOSTNAME=localhost;PORT=50000;DATABASE=doctrine;UID=db2inst1;PWD=Passw0rd",
        ),
    ]
}

#[test]
fn testFromConnectionParameters() {
    for (case, params, expected) in connection_parameters_provider() {
        let dsn = DataSourceName::from_connection_parameters(&params).unwrap();

        assert_eq!(&PhpString::from(expected), dsn.to_php_string(), "{case}");
    }
}
