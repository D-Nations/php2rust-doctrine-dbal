//! Port of Doctrine\DBAL\Driver\API\MySQL\ExceptionConverter.php
//! (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/API/MySQL/ExceptionConverter.php

use super::ExceptionConverter as ExceptionConverterInterface;
use crate::{Exception, Query};

/// The `MySQL` exception converter: it reads the error code
/// (<https://dev.mysql.com/doc/mysql-errors/8.0/en/client-error-reference.html>,
/// <https://dev.mysql.com/doc/mysql-errors/8.0/en/server-error-reference.html>).
#[derive(Debug, Clone, Copy, Default)]
pub struct ExceptionConverter;

/// `str_contains()` on bytes.
fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

impl ExceptionConverterInterface for ExceptionConverter {
    fn convert(&self, exception: Exception, query: Option<Query>) -> Exception {
        let code = exception.code();
        if code == 1524
            && contains(
                exception.message().as_bytes(),
                "Plugin 'mysql_native_password' is not loaded",
            )
        {
            // Workaround for MySQL 8.4 if we request an unknown user.
            // https://bugs.mysql.com/bug.php?id=114876
            return Exception::driver_exception(
                Exception::CONNECTION_EXCEPTION_DRIVER,
                exception,
                query,
            );
        }
        let class = match code {
            1008 => Exception::DATABASE_DOES_NOT_EXIST,
            1213 => Exception::DEADLOCK_EXCEPTION,
            1205 => Exception::LOCK_WAIT_TIMEOUT_EXCEPTION,
            1050 => Exception::TABLE_EXISTS_EXCEPTION,
            1051 | 1146 => Exception::TABLE_NOT_FOUND_EXCEPTION,
            1216 | 1217 | 1451 | 1452 | 1701 => {
                Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION
            }
            1062 | 1557 | 1569 | 1586 => Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION,
            1054 | 1166 | 1611 => Exception::INVALID_FIELD_NAME_EXCEPTION,
            1052 | 1060 | 1110 => Exception::NON_UNIQUE_FIELD_NAME_EXCEPTION,
            1064 | 1149 | 1287 | 1341 | 1342 | 1343 | 1344 | 1382 | 1479 | 1541 | 1554 | 1626 => {
                Exception::SYNTAX_ERROR_EXCEPTION
            }
            1044 | 1045 | 1046 | 1049 | 1095 | 1142 | 1143 | 1227 | 1370 | 1429 | 2002 | 2005
            | 2054 => Exception::CONNECTION_EXCEPTION_DRIVER,
            2006 | 4031 => Exception::CONNECTION_LOST,
            1048 | 1121 | 1138 | 1171 | 1252 | 1263 | 1364 | 1566 => {
                Exception::NOT_NULL_CONSTRAINT_VIOLATION_EXCEPTION
            }
            _ => Exception::DRIVER_EXCEPTION,
        };
        Exception::driver_exception(class, exception, query)
    }
}
