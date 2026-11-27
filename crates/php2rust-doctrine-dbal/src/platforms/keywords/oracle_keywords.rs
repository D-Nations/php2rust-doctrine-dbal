//! Port of Doctrine\DBAL\Platforms\Keywords\OracleKeywords.php (deprecated
//! upstream).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/OracleKeywords.php

use super::KeywordList;

/// Oracle Keywordlist: `new OracleKeywords()`.
pub struct OracleKeywords;

impl OracleKeywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\OracleKeywords";

    /// `new OracleKeywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`.
    #[allow(clippy::too_many_lines, reason = "the upstream keyword list")]
    pub fn get_keywords() -> &'static [&'static str] {
        &[
            "ACCESS",
            "ADD",
            "ALL",
            "ALTER",
            "AND",
            "ANY",
            "ARRAYLEN",
            "AS",
            "ASC",
            "AUDIT",
            "BETWEEN",
            "BY",
            "CHAR",
            "CHECK",
            "CLUSTER",
            "COLUMN",
            "COMMENT",
            "COMPRESS",
            "CONNECT",
            "CREATE",
            "CURRENT",
            "DATE",
            "DECIMAL",
            "DEFAULT",
            "DELETE",
            "DESC",
            "DISTINCT",
            "DROP",
            "ELSE",
            "EXCLUSIVE",
            "EXISTS",
            "FILE",
            "FLOAT",
            "FOR",
            "FROM",
            "GRANT",
            "GROUP",
            "HAVING",
            "IDENTIFIED",
            "IMMEDIATE",
            "IN",
            "INCREMENT",
            "INDEX",
            "INITIAL",
            "INSERT",
            "INTEGER",
            "INTERSECT",
            "INTO",
            "IS",
            "LEVEL",
            "LIKE",
            "LOCK",
            "LONG",
            "MAXEXTENTS",
            "MINUS",
            "MODE",
            "MODIFY",
            "NOAUDIT",
            "NOCOMPRESS",
            "NOT",
            "NOTFOUND",
            "NOWAIT",
            "NULL",
            "NUMBER",
            "OF",
            "OFFLINE",
            "ON",
            "ONLINE",
            "OPTION",
            "OR",
            "ORDER",
            "PCTFREE",
            "PRIOR",
            "PRIVILEGES",
            "PUBLIC",
            "RANGE",
            "RAW",
            "RENAME",
            "RESOURCE",
            "REVOKE",
            "ROW",
            "ROWID",
            "ROWLABEL",
            "ROWNUM",
            "ROWS",
            "SELECT",
            "SESSION",
            "SET",
            "SHARE",
            "SIZE",
            "SMALLINT",
            "SQLBUF",
            "START",
            "SUCCESSFUL",
            "SYNONYM",
            "SYSDATE",
            "TABLE",
            "THEN",
            "TO",
            "TRIGGER",
            "UID",
            "UNION",
            "UNIQUE",
            "UPDATE",
            "USER",
            "VALIDATE",
            "VALUES",
            "VARCHAR",
            "VARCHAR2",
            "VIEW",
            "WHENEVER",
            "WHERE",
            "WITH",
        ]
    }
}
