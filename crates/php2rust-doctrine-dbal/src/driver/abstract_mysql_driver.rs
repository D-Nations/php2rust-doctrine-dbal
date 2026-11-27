//! Port of Doctrine\DBAL\Driver\AbstractMySQLDriver.php: the base of the
//! `MySQL` drivers, which picks the platform from the server version. As
//! with [`super::AbstractSQLiteDriver`], the abstract class is a trait whose
//! provided methods are the class's methods; a `MySQL` driver forwards its
//! `Driver` methods to them.
//!
//! PHP's `version_compare()` is [`version_compare`] here, and the
//! `MariaDB` version is read without a regex, as the pattern of
//! `getMariaDbMysqlVersionNumber()` matches.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/AbstractMySQLDriver.php

use std::cmp::Ordering;
use std::sync::Arc;

use super::api::{ExceptionConverter, mysql};
use super::{BoxFuture, PlatformBox};
use crate::platforms::{
    MariaDB1010Platform, MariaDB1052Platform, MariaDB1060Platform, MariaDB110700Platform,
    MariaDB120300Platform, MariaDBPlatform, MySQL80Platform, MySQL84Platform, MySQLPlatform,
};
use crate::{Exception, ServerVersionProvider, deprecations};

/// Abstract base implementation of the `Driver` interface for `MySQL` based
/// drivers.
pub trait AbstractMySQLDriver {
    /// `getDatabasePlatform($versionProvider)`: the `MySQL` or `MariaDB`
    /// platform of the server version.
    fn mysql_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async move {
            let version = version_provider.get_server_version().await?;
            mysql_platform_for_version(&version)
        })
    }

    /// `getExceptionConverter()`.
    fn mysql_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(mysql::ExceptionConverter)
    }
}

const DEPRECATION_LINK: &str = "https://github.com/doctrine/dbal/pull/6343";

/// The platform `getDatabasePlatform()` returns for a server version.
pub fn mysql_platform_for_version(version: &str) -> Result<PlatformBox, Exception> {
    let at_least = |version: &str, minimum: &str| version_compare(version, minimum).is_ge();

    if version.to_ascii_lowercase().contains("mariadb") {
        let mariadb_version = mariadb_mysql_version_number(version)?;
        if at_least(&mariadb_version, "12.3.0") {
            return Ok(Box::new(MariaDB120300Platform::new()));
        }
        if at_least(&mariadb_version, "11.7.0") {
            return Ok(Box::new(MariaDB110700Platform::new()));
        }
        if at_least(&mariadb_version, "10.10.0") {
            return Ok(Box::new(MariaDB1010Platform::new()));
        }
        if at_least(&mariadb_version, "10.6.0") {
            return Ok(Box::new(MariaDB1060Platform::new()));
        }
        deprecations::trigger(DEPRECATION_LINK, || {
            "Support for MariaDB < 10.6.0 is deprecated and will be removed in DBAL 5".to_owned()
        });
        if at_least(&mariadb_version, "10.5.2") {
            return Ok(Box::new(MariaDB1052Platform::new()));
        }
        return Ok(Box::new(MariaDBPlatform::new()));
    }

    if at_least(version, "8.4.0") {
        return Ok(Box::new(MySQL84Platform::new()));
    }
    if at_least(version, "8.0.0") {
        return Ok(Box::new(MySQL80Platform::new()));
    }
    deprecations::trigger(DEPRECATION_LINK, || {
        "Support for MySQL < 8 is deprecated and will be removed in DBAL 5".to_owned()
    });
    Ok(Box::new(MySQLPlatform::new()))
}

/// `getMariaDbMysqlVersionNumber()`: `major.minor.patch` as the pattern
/// `/^(?:5\.5\.5-)?(mariadb-)?(?P<major>\d+)\.(?P<minor>\d+)\.(?P<patch>\d+)/i`
/// reads it.
fn mariadb_mysql_version_number(version_string: &str) -> Result<String, Exception> {
    /// The pattern from `rest` on, after the optional prefixes.
    fn numbers(rest: &str) -> Option<String> {
        let mut parts = Vec::with_capacity(3);
        let mut rest = rest;
        for i in 0..3 {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            if digits == 0 {
                return None;
            }
            parts.push(&rest[..digits]);
            rest = &rest[digits..];
            if i < 2 {
                rest = rest.strip_prefix('.')?;
            }
        }
        Some(parts.join("."))
    }

    fn strip_prefix_ignore_case<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
        (s.len() >= prefix.len()
            && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes()))
        .then(|| &s[prefix.len()..])
    }

    // The regex backtracks over its optional groups: try each way of
    // taking them, the greedy ones first.
    let after_555 = [
        strip_prefix_ignore_case(version_string, "5.5.5-"),
        Some(version_string),
    ];
    for start in after_555.into_iter().flatten() {
        let candidates = [strip_prefix_ignore_case(start, "mariadb-"), Some(start)];
        for rest in candidates.into_iter().flatten() {
            if let Some(number) = numbers(rest) {
                return Ok(number);
            }
        }
    }
    Err(Exception::invalid_platform_version(
        version_string,
        "^(?:5\\.5\\.5-)?(mariadb-)?<major_version>.<minor_version>.<patch_version>",
    ))
}

/// PHP's `version_compare($version1, $version2)`: the versions are
/// canonicalized (`-`, `_` and `+` become `.`, and a `.` separates a run of
/// digits from one of other characters), then compared part by part,
/// numbers as numbers and the special forms in the order `dev` < `alpha`
/// = `a` < `beta` = `b` < `RC` = `rc` < `#` (a number) < `pl` = `p`.
pub fn version_compare(version1: &str, version2: &str) -> Ordering {
    match (version1.is_empty(), version2.is_empty()) {
        (true, true) => return Ordering::Equal,
        (true, false) => return Ordering::Less,
        (false, true) => return Ordering::Greater,
        (false, false) => {}
    }
    let v1 = canonicalize(version1.as_bytes());
    let v2 = canonicalize(version2.as_bytes());
    compare_canonical(&v1, &v2)
}

fn compare_canonical(v1: &[u8], v2: &[u8]) -> Ordering {
    let parts1: Vec<&[u8]> = v1.split(|&b| b == b'.').collect();
    let parts2: Vec<&[u8]> = v2.split(|&b| b == b'.').collect();
    let mut index = 0;
    let mut compare = Ordering::Equal;
    while index < parts1.len() && index < parts2.len() && compare == Ordering::Equal {
        let (p1, p2) = (parts1[index], parts2[index]);
        let digit1 = p1.first().is_some_and(u8::is_ascii_digit);
        let digit2 = p2.first().is_some_and(u8::is_ascii_digit);
        compare = match (digit1, digit2) {
            (true, true) => strtol(p1).cmp(&strtol(p2)),
            (false, false) => compare_special_version_forms(p1, p2),
            (true, false) => compare_special_version_forms(b"#N#", p2),
            (false, true) => compare_special_version_forms(p1, b"#N#"),
        };
        index += 1;
    }
    if compare == Ordering::Equal {
        if index < parts1.len() {
            compare = if parts1[index].first().is_some_and(u8::is_ascii_digit) {
                Ordering::Greater
            } else {
                compare_canonical(&parts1[index..].join(&b'.'), b"#N#")
            };
        } else if index < parts2.len() {
            compare = if parts2[index].first().is_some_and(u8::is_ascii_digit) {
                Ordering::Less
            } else {
                compare_canonical(b"#N#", &parts2[index..].join(&b'.'))
            };
        }
    }
    compare
}

/// `strtol($part, NULL, 10)` of a part that starts with a digit.
fn strtol(part: &[u8]) -> i64 {
    part.iter()
        .take_while(|b| b.is_ascii_digit())
        .fold(0i64, |n, &b| {
            n.saturating_mul(10).saturating_add(i64::from(b - b'0'))
        })
}

fn compare_special_version_forms(form1: &[u8], form2: &[u8]) -> Ordering {
    const FORMS: [(&[u8], i32); 10] = [
        (b"dev", 0),
        (b"alpha", 1),
        (b"a", 1),
        (b"beta", 2),
        (b"b", 2),
        (b"RC", 3),
        (b"rc", 3),
        (b"#", 4),
        (b"pl", 5),
        (b"p", 5),
    ];
    let order = |form: &[u8]| {
        FORMS
            .iter()
            .find(|(name, _)| form.starts_with(name))
            .map_or(-1, |(_, order)| *order)
    };
    order(form1).cmp(&order(form2))
}

/// `php_canonicalize_version()`.
fn canonicalize(version: &[u8]) -> Vec<u8> {
    let digit = |b: u8| b.is_ascii_digit();
    let other = |b: u8| !b.is_ascii_digit() && b != b'.';
    let mut out = vec![version[0]];
    let mut last = version[0];
    for &b in &version[1..] {
        let previous = *out.last().unwrap_or(&0);
        if matches!(b, b'-' | b'_' | b'+') {
            if previous != b'.' {
                out.push(b'.');
            }
        } else if (other(last) && digit(b)) || (digit(last) && other(b)) {
            if previous != b'.' {
                out.push(b'.');
            }
            out.push(b);
        } else if !b.is_ascii_alphanumeric() {
            if previous != b'.' {
                out.push(b'.');
            }
        } else {
            out.push(b);
        }
        last = b;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_as_php() {
        // Results of PHP 8.4's version_compare().
        let cases = [
            ("8.0.11", "8.0.0", Ordering::Greater),
            ("8.4.1", "8.4.0", Ordering::Greater),
            ("5.7.0", "8.0.0", Ordering::Less),
            ("9.0.0", "8.4.0", Ordering::Greater),
            ("8.4.0", "8.4.0", Ordering::Equal),
            ("8.4", "8.4.0", Ordering::Less),
            ("8.4.0-log", "8.4.0", Ordering::Less),
            ("1.0rc1", "1.0", Ordering::Less),
            ("1.0pl1", "1.0", Ordering::Greater),
            ("1.0.0-dev", "1.0.0alpha", Ordering::Less),
            ("1.0", "1.0.0", Ordering::Less),
            ("", "1", Ordering::Less),
        ];
        for (v1, v2, expected) in cases {
            assert_eq!(version_compare(v1, v2), expected, "{v1} <=> {v2}");
        }
    }

    #[test]
    fn reads_the_mariadb_version() {
        assert_eq!(
            mariadb_mysql_version_number("5.5.5-MariaDB-10.2.8+maria~xenial-log").unwrap(),
            "10.2.8"
        );
        assert_eq!(
            mariadb_mysql_version_number("5.5.40-MariaDB-1~wheezy").unwrap(),
            "5.5.40"
        );
        assert_eq!(
            mariadb_mysql_version_number("mariadb-10.9.3").unwrap(),
            "10.9.3"
        );
        assert!(mariadb_mysql_version_number("MariaDB-x").is_err());
    }
}
