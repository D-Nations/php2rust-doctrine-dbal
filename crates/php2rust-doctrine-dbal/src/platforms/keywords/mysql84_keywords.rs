//! Port of Doctrine\DBAL\Platforms\Keywords\MySQL84Keywords.php (deprecated
//! upstream).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Keywords/MySQL84Keywords.php

use std::sync::OnceLock;

use super::KeywordList;
use super::mysql80_keywords::MySQL80Keywords;

/// `MySQL` 8.4 reserved keywords list: `new MySQL84Keywords()`.
pub struct MySQL84Keywords;

impl MySQL84Keywords {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Keywords\\MySQL84Keywords";

    /// `new MySQL84Keywords()`.
    #[allow(clippy::new_ret_no_self, reason = "the list is a KeywordList")]
    pub fn new() -> KeywordList {
        KeywordList::new(Self::CLASS, Self::get_keywords)
    }

    /// `getKeywords()`: `parent::getKeywords()` with this version's
    /// changes.
    pub fn get_keywords() -> &'static [&'static str] {
        static KEYWORDS: OnceLock<Vec<&'static str>> = OnceLock::new();
        KEYWORDS.get_or_init(|| {
            let mut keywords = MySQL80Keywords::get_keywords().to_vec();
            // Removed Keywords and Reserved Words
            let removed = ["MASTER_BIND", "MASTER_SSL_VERIFY_SERVER_CERT"];
            keywords.retain(|keyword| !removed.contains(keyword));
            keywords.extend([
                "AUTO",
                "BERNOULLI",
                "GTIDS",
                "LOG",
                "MANUAL",
                "PARALLEL",
                "PARSE_TREE",
                "QUALIFY",
                "S3",
                "TABLESAMPLE",
            ]);
            keywords
        })
    }
}
