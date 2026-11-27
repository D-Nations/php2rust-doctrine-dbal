//! Port of tests/Schema/AbstractAssetTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; the anonymous subclasses of `AbstractAsset` are
//! `AbstractAsset::new()` with `AssetKind::Bare`. The providers'
//! platforms are DBAL's `OraclePlatform`, `PostgreSQLPlatform` and
//! `MySQLPlatform`.
//!
//! Upstream: doctrine/dbal tests/Schema/AbstractAssetTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod schema_support;

use php2rust_doctrine_dbal::platforms::{
    AbstractPlatform, MySQLPlatform, OraclePlatform, PostgreSQLPlatform,
};
use php2rust_doctrine_dbal::schema::{AbstractAsset, AssetKind, Identifier, NameParser};
use schema_support::{expect_deprecation, expect_no_deprecation};

const CLASS: &str = "Doctrine\\DBAL\\Schema\\AbstractAsset@anonymous";

#[test]
fn testConstructWithoutArguments() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6610", || {
        AbstractAsset::new(
            CLASS,
            AssetKind::Bare,
            NameParser::NotImplemented(CLASS),
            None,
        );
    });
}

#[test]
fn testCreateParserNotImplemented() {
    expect_deprecation("https://github.com/doctrine/dbal/pull/6592", || {
        AbstractAsset::new(
            CLASS,
            AssetKind::Bare,
            NameParser::NotImplemented(CLASS),
            Some("foo"),
        );
    });
}

/// A provider row's platform.
fn platform(name: &str) -> Box<dyn AbstractPlatform> {
    match name {
        "Oracle" => Box::new(OraclePlatform::new()),
        "PostgreSQL" => Box::new(PostgreSQLPlatform::new()),
        _ => Box::new(MySQLPlatform::new()),
    }
}

/// `nameParsingDeprecationProvider()`.
fn name_parsing_deprecation_provider() -> [(&'static str, &'static str); 5] {
    [
        // unquoted keywords not in normal case
        ("select", "Oracle"),
        ("SELECT", "PostgreSQL"),
        // unquoted name not in normal case qualified by quoted name
        ("\"_\".id", "Oracle"),
        ("\"_\".ID", "PostgreSQL"),
        // incompatible parser behavior
        ("\"example.com\"", "MySQL"),
    ]
}

#[test]
fn testNameParsingDeprecation() {
    for (name, platform_name) in name_parsing_deprecation_provider() {
        let platform = platform(platform_name);
        expect_deprecation("https://github.com/doctrine/dbal/pull/6592", || {
            let identifier = Identifier::new(name, false);
            identifier.get_quoted_name(&*platform);
        });
    }
}

/// `noNameParsingDeprecationProvider()`.
fn no_name_parsing_deprecation_provider() -> [(&'static str, &'static str); 12] {
    [
        // empty name
        ("", "MySQL"),
        // name with one qualifier
        ("schema.table", "MySQL"),
        // quoted keywords
        ("\"select\"", "Oracle"),
        ("\"SELECT\"", "PostgreSQL"),
        // unquoted keywords in normal case
        ("SELECT", "Oracle"),
        ("select", "PostgreSQL"),
        // unquoted keywords in any case on a platform that does not force a case
        ("SELECT", "MySQL"),
        ("select", "MySQL"),
        // non-keywords in any case
        ("id", "Oracle"),
        ("ID", "Oracle"),
        ("id", "PostgreSQL"),
        ("ID", "PostgreSQL"),
    ]
}

#[test]
fn testNoNameParsingDeprecation() {
    for (name, platform_name) in no_name_parsing_deprecation_provider() {
        let platform = platform(platform_name);
        let identifier = Identifier::new(name, false);

        expect_no_deprecation("https://github.com/doctrine/dbal/pull/6592", || {
            identifier.get_quoted_name(&*platform);
        });
    }
}
