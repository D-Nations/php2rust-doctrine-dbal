//! Port of Doctrine\DBAL\Platforms\Keywords\PostgreSQLKeywords.php (deprecated
//! upstream).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/PostgreSQLKeywords.php

use super::KeywordList;

/// Reserved keywords list corresponding to the `PostgreSQL` database
/// platform of the oldest supported version: `new PostgreSQLKeywords()`.
pub struct PostgreSQLKeywords;

impl PostgreSQLKeywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\PostgreSQLKeywords";

    /// `new PostgreSQLKeywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`.
    #[allow(clippy::too_many_lines, reason = "the upstream keyword list")]
    pub fn get_keywords() -> &'static [&'static str] {
        &[
            "ALL",
            "ANALYSE",
            "ANALYZE",
            "AND",
            "ANY",
            "ARRAY",
            "AS",
            "ASC",
            "ASYMMETRIC",
            "AUTHORIZATION",
            "BINARY",
            "BOTH",
            "CASE",
            "CAST",
            "CHECK",
            "COLLATE",
            "COLLATION",
            "COLUMN",
            "CONCURRENTLY",
            "CONSTRAINT",
            "CREATE",
            "CROSS",
            "CURRENT_CATALOG",
            "CURRENT_DATE",
            "CURRENT_ROLE",
            "CURRENT_SCHEMA",
            "CURRENT_TIME",
            "CURRENT_TIMESTAMP",
            "CURRENT_USER",
            "DEFAULT",
            "DEFERRABLE",
            "DESC",
            "DISTINCT",
            "DO",
            "ELSE",
            "END",
            "EXCEPT",
            "FALSE",
            "FETCH",
            "FOR",
            "FOREIGN",
            "FREEZE",
            "FROM",
            "FULL",
            "GRANT",
            "GROUP",
            "HAVING",
            "ILIKE",
            "IN",
            "INITIALLY",
            "INNER",
            "INTERSECT",
            "INTO",
            "IS",
            "ISNULL",
            "JOIN",
            "LATERAL",
            "LEADING",
            "LEFT",
            "LIKE",
            "LIMIT",
            "LOCALTIME",
            "LOCALTIMESTAMP",
            "NATURAL",
            "NOT",
            "NOTNULL",
            "NULL",
            "OFFSET",
            "ON",
            "ONLY",
            "OR",
            "ORDER",
            "OUTER",
            "OVERLAPS",
            "PLACING",
            "PRIMARY",
            "REFERENCES",
            "RETURNING",
            "RIGHT",
            "SELECT",
            "SESSION_USER",
            "SIMILAR",
            "SOME",
            "SYMMETRIC",
            "TABLE",
            "THEN",
            "TO",
            "TRAILING",
            "TRUE",
            "UNION",
            "UNIQUE",
            "USER",
            "USING",
            "VARIADIC",
            "VERBOSE",
            "WHEN",
            "WHERE",
            "WINDOW",
            "WITH",
        ]
    }
}
