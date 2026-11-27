//! Port of Doctrine\DBAL\Platforms\Keywords\MySQL80Keywords.php (deprecated
//! upstream).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/MySQL80Keywords.php

use std::sync::OnceLock;

use super::KeywordList;
use super::mysql_keywords::MySQLKeywords;

/// `MySQL` 8.0 reserved keywords list: `new MySQL80Keywords()`.
pub struct MySQL80Keywords;

impl MySQL80Keywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\MySQL80Keywords";

    /// `new MySQL80Keywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`: `parent::getKeywords()` with this version's
    /// changes.
    pub fn get_keywords() -> &'static [&'static str] {
        static KEYWORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
        KEYWORDS.get_or_init(|| {
            let mut keywords = MySQLKeywords::get_keywords().to_vec();
            keywords.extend([
                "ADMIN",
                "ARRAY",
                "CUBE",
                "CUME_DIST",
                "DENSE_RANK",
                "EMPTY",
                "EXCEPT",
                "FIRST_VALUE",
                "FUNCTION",
                "GROUPING",
                "GROUPS",
                "JSON_TABLE",
                "LAG",
                "LAST_VALUE",
                "LATERAL",
                "LEAD",
                "MEMBER",
                "NTH_VALUE",
                "NTILE",
                "OF",
                "OVER",
                "PERCENT_RANK",
                "PERSIST",
                "PERSIST_ONLY",
                "RANK",
                "RECURSIVE",
                "ROW",
                "ROWS",
                "ROW_NUMBER",
                "SYSTEM",
                "WINDOW",
            ]);
            keywords
        })
    }
}
