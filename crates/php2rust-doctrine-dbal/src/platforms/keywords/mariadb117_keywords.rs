//! Port of Doctrine\DBAL\Platforms\Keywords\MariaDB117Keywords.php
//! (deprecated upstream).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/MariaDB117Keywords.php

use std::sync::OnceLock;

use super::KeywordList;
use super::mariadb_keywords::MariaDBKeywords;

/// `MariaDB` 11.7 reserved keywords list: `new MariaDB117Keywords()`.
pub struct MariaDB117Keywords;

impl MariaDB117Keywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\MariaDB117Keywords";

    /// `new MariaDB117Keywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`: `parent::getKeywords()` with this version's
    /// changes.
    pub fn get_keywords() -> &'static [&'static str] {
        static KEYWORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
        KEYWORDS.get_or_init(|| {
            let mut keywords = MariaDBKeywords::get_keywords().to_vec();
            // New Keywords and Reserved Words
            keywords.push("VECTOR");
            keywords
        })
    }
}
