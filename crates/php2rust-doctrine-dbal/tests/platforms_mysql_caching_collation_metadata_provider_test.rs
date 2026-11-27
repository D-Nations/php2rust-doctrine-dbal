//! Port of tests/Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProviderTest.php
//! at doctrine/dbal c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and
//! assertions are PHPUnit's. The mock is a provider that records the
//! collations it is asked for: `expects(self::once())->with($collation)`
//! is one recorded call with that collation. The test runs every case of
//! `charsetAndCollationProvider()`.
//!
//! Upstream: doctrine/dbal tests/Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProviderTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use php2rust_doctrine_dbal::Exception;
use php2rust_doctrine_dbal::platforms::mysql::{
    CachingCollationMetadataProvider, CollationMetadataProvider,
};

/// `createMock(CollationMetadataProvider::class)` with
/// `->method('getCollationCharset')->willReturn($charset)`.
struct CollationMetadataProviderMock {
    charset: Option<String>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl CollationMetadataProvider for CollationMetadataProviderMock {
    fn get_collation_charset(&self, collation: &str) -> Result<Option<String>, Exception> {
        self.calls.borrow_mut().push(collation.to_owned());
        Ok(self.charset.clone())
    }
}

/// The body of `testCharsetCaching($collation, $charset)`.
fn charset_caching(collation: &str, charset: Option<&str>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let underlying_provider = CollationMetadataProviderMock {
        charset: charset.map(str::to_owned),
        calls: Rc::clone(&calls),
    };

    let caching_provider = CachingCollationMetadataProvider::new(Box::new(underlying_provider));
    assert_eq!(
        charset,
        caching_provider
            .get_collation_charset(collation)
            .unwrap()
            .as_deref()
    );
    assert_eq!(
        charset,
        caching_provider
            .get_collation_charset(collation)
            .unwrap()
            .as_deref()
    );

    assert_eq!(vec![collation.to_owned()], *calls.borrow());
}

/// `charsetAndCollationProvider()`.
const CHARSET_AND_COLLATION_PROVIDER: [(&str, &str, Option<&str>); 2] = [
    ("found", "utf8mb4_unicode_ci", Some("utf8mb4")),
    ("not found", "utf8mb5_unicode_ci", None),
];

#[test]
fn testCharsetCaching() {
    for (_case, collation, charset) in CHARSET_AND_COLLATION_PROVIDER {
        charset_caching(collation, charset);
    }
}
