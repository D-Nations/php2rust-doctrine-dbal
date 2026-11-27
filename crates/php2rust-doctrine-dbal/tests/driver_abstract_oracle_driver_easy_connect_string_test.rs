//! Port of tests/Driver/AbstractOracleDriver/EasyConnectStringTest.php at
//! doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; a data provider's cases run in one test, in
//! order. `VerifyDeprecations` is `schema_support`'s
//! `expect_deprecation()` and `expect_no_deprecation()`.
//!
//! Upstream: doctrine/dbal tests/Driver/AbstractOracleDriver/EasyConnectStringTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php_value::{PhpArray, PhpString, Value};
use php2rust_doctrine_dbal::driver::abstract_oracle_driver::EasyConnectString;
use schema_support::{expect_deprecation, expect_no_deprecation};

fn params(pairs: &[(&str, Value)]) -> PhpArray {
    let mut params = PhpArray::new();
    for (key, value) in pairs {
        params.insert(*key, value.clone());
    }
    params
}

fn s(value: &str) -> Value {
    Value::string(value)
}

/// `connectionParametersProvider()`.
fn connection_parameters_provider() -> Vec<(&'static str, PhpArray, &'static str)> {
    let mut driver_options = PhpArray::new();
    driver_options.insert("protocol", s("TCPS"));

    vec![
        (
            "common-params",
            params(&[
                ("host", s("oracle.example.com")),
                ("port", Value::Int(1521)),
                ("dbname", s("XE")),
            ]),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=oracle.example.com)(PORT=1521))(CONNECT_DATA=(SID=XE)))",
        ),
        (
            "no-db-name",
            params(&[("host", s("localhost"))]),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=1521)))",
        ),
        (
            "service",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(1521)),
                ("service", Value::Bool(true)),
                ("servicename", s("BILLING")),
            ]),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=BILLING)))",
        ),
        (
            "advanced-params",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(41521)),
                ("dbname", s("XE")),
                ("instancename", s("SALES")),
                ("pooled", Value::Bool(true)),
            ]),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=41521))(CONNECT_DATA=(SID=XE)(INSTANCE_NAME=SALES)(SERVER=POOLED)))",
        ),
        (
            "tcps-params",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(41521)),
                ("dbname", s("XE")),
                ("instancename", s("SALES")),
                ("pooled", Value::Bool(true)),
                ("driverOptions", Value::Array(driver_options)),
            ]),
            "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCPS)(HOST=localhost)(PORT=41521))(CONNECT_DATA=(SID=XE)(INSTANCE_NAME=SALES)(SERVER=POOLED)))",
        ),
    ]
}

#[test]
fn testFromConnectionParameters() {
    for (case, params, expected) in connection_parameters_provider() {
        let string = EasyConnectString::from_connection_parameters(&params).unwrap();

        assert_eq!(&PhpString::from(expected), string.to_php_string(), "{case}");
    }
}

/// `getConnectionParameters()`.
fn get_connection_parameters() -> Vec<(&'static str, PhpArray, &'static str, &'static str, bool)> {
    let service_name_string = "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=1521))(CONNECT_DATA=(SERVICE_NAME=BILLING)))";
    let sid_string = "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=localhost)(PORT=1521))(CONNECT_DATA=(SID=BILLING)))";

    vec![
        (
            "service",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(1521)),
                ("dbname", s("BILLING")),
                ("service", Value::Bool(true)),
            ]),
            service_name_string,
            "https://github.com/doctrine/dbal/pull/7239",
            true,
        ),
        (
            "dbname",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(1521)),
                ("dbname", s("BILLING")),
            ]),
            sid_string,
            "https://github.com/doctrine/dbal/pull/7239",
            true,
        ),
        (
            "servicename",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(1521)),
                ("servicename", s("BILLING")),
            ]),
            service_name_string,
            "https://github.com/doctrine/dbal/pull/7042",
            false,
        ),
        (
            "sid",
            params(&[
                ("host", s("localhost")),
                ("port", Value::Int(1521)),
                ("sid", s("BILLING")),
            ]),
            sid_string,
            "https://github.com/doctrine/dbal/pull/7042",
            false,
        ),
    ]
}

#[test]
fn testParameterDeprecation() {
    for (case, parameters, expected_connect_string, deprecation, expect) in
        get_connection_parameters()
    {
        let create = || EasyConnectString::from_connection_parameters(&parameters).unwrap();
        let string = if expect {
            expect_deprecation(deprecation, create)
        } else {
            expect_no_deprecation(deprecation, create)
        };

        assert_eq!(
            &PhpString::from(expected_connect_string),
            string.to_php_string(),
            "{case}"
        );
    }
}

#[test]
fn testNoHostOrConnectStringSpecified() {
    let string = expect_deprecation("https://github.com/doctrine/dbal/pull/7244", || {
        EasyConnectString::from_connection_parameters(&PhpArray::new()).unwrap()
    });

    assert_eq!(&PhpString::from(""), string.to_php_string());
}
