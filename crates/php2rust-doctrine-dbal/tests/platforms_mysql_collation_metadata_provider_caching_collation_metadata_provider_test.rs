//! Port of
//! tests/Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProviderTest.php
//! at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's; the data provider's sets run in one test.
//! The mock (`expects(self::once())->method('getCollationCharset')
//! ->with($collation)`) counts its calls and checks the argument.
//!
//! Upstream: doctrine/dbal tests/Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProviderTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::platforms::mysql::{
    CachingCollationMetadataProvider, CollationMetadataProvider,
};

struct UnderlyingProvider {
    collation: &'static str,
    charset: Option<&'static str>,
    calls: Rc<Cell<usize>>,
}

impl CollationMetadataProvider for UnderlyingProvider {
    fn get_collation_charset(&self, collation: &str) -> Result<Option<String>, Exception> {
        self.calls.set(self.calls.get() + 1);
        assert_eq!(collation, self.collation);
        Ok(self.charset.map(str::to_owned))
    }
}

/// `charsetAndCollationProvider()`.
fn charset_and_collation_provider() -> [(&'static str, &'static str, Option<&'static str>); 2] {
    [
        ("found", "utf8mb4_unicode_ci", Some("utf8mb4")),
        ("not found", "utf8mb5_unicode_ci", None),
    ]
}

#[test]
fn testCharsetCaching() {
    for (set, collation, charset) in charset_and_collation_provider() {
        let calls = Rc::new(Cell::new(0));
        let underlying_provider = UnderlyingProvider {
            collation,
            charset,
            calls: calls.clone(),
        };

        let caching_provider = CachingCollationMetadataProvider::new(Box::new(underlying_provider));
        assert_eq!(
            caching_provider
                .get_collation_charset(collation)
                .unwrap()
                .as_deref(),
            charset,
            "{set}"
        );
        assert_eq!(
            caching_provider
                .get_collation_charset(collation)
                .unwrap()
                .as_deref(),
            charset,
            "{set}"
        );

        assert_eq!(
            calls.get(),
            1,
            "{set}: getCollationCharset() is called once"
        );
    }
}
