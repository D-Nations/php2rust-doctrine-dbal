//! Port of Doctrine\DBAL\Driver\API\PostgreSQL\ExceptionConverter.php
//! (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/API/PostgreSQL/ExceptionConverter.php

use super::ExceptionConverter as ExceptionConverterInterface;
use crate::{Exception, Query};

/// The `PostgreSQL` exception converter: it reads the `SQLSTATE`
/// (<http://www.postgresql.org/docs/9.4/static/errcodes-appendix.html>).
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
        let message = exception.message().as_bytes();
        let class = match exception.sql_state() {
            Some("40001" | "40P01") => Some(Exception::DEADLOCK_EXCEPTION),
            // Foreign key constraint violations during a TRUNCATE operation
            // are considered "feature not supported" in PostgreSQL.
            Some("0A000") if contains(message, "truncate") => {
                Some(Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION)
            }
            Some("23502") => Some(Exception::NOT_NULL_CONSTRAINT_VIOLATION_EXCEPTION),
            Some("23503") => Some(Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION),
            Some("23505") => Some(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION),
            Some("3D000") => Some(Exception::DATABASE_DOES_NOT_EXIST),
            Some("3F000") => Some(Exception::SCHEMA_DOES_NOT_EXIST),
            Some("42601") => Some(Exception::SYNTAX_ERROR_EXCEPTION),
            Some("42702") => Some(Exception::NON_UNIQUE_FIELD_NAME_EXCEPTION),
            Some("42703") => Some(Exception::INVALID_FIELD_NAME_EXCEPTION),
            Some("42P01") => Some(Exception::TABLE_NOT_FOUND_EXCEPTION),
            Some("42P07") => Some(Exception::TABLE_EXISTS_EXCEPTION),
            Some("08006") => Some(Exception::CONNECTION_EXCEPTION_DRIVER),
            _ => None,
        };

        let class = class.unwrap_or_else(|| {
            if contains(message, "terminating connection")
                || contains(message, "server closed the connection")
            {
                Exception::CONNECTION_LOST
            } else {
                Exception::DRIVER_EXCEPTION
            }
        });

        Exception::driver_exception(class, exception, query)
    }
}
