//! Port of Doctrine\DBAL\Platforms\Keywords\MariaDB123Keywords.php
//! (deprecated upstream).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/MariaDB123Keywords.php

use std::sync::OnceLock;

use super::KeywordList;
use super::mariadb117_keywords::MariaDB117Keywords;

/// `MariaDB` 12.3 reserved keywords list: `new MariaDB123Keywords()`.
pub struct MariaDB123Keywords;

impl MariaDB123Keywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\MariaDB123Keywords";

    /// `new MariaDB123Keywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`: `parent::getKeywords()` with this version's
    /// changes.
    pub fn get_keywords() -> &'static [&'static str] {
        static KEYWORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
        KEYWORDS.get_or_init(|| {
            let mut keywords = MariaDB117Keywords::get_keywords().to_vec();
            // TO_DATE() was added as an Oracle-compatible function in MariaDB 12.3
            // and is implemented as a parser-level keyword, so it can no longer be
            // used as an unquoted identifier.
            keywords.push("TO_DATE");
            keywords
        })
    }
}
