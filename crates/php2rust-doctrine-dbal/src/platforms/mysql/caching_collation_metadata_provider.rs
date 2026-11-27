//! Port of Doctrine\DBAL\Platforms\MySQL\CollationMetadataProvider\CachingCollationMetadataProvider.php.
//!
//! `@internal`. The cache is behind a `RefCell` because
//! `CollationMetadataProvider::get_collation_charset()` takes `&self`.
//! A `null` charset is cached like any other answer; an exception from the
//! underlying provider is passed on and nothing is cached, as in PHP.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProvider.php

use std::cell::RefCell;
use std::collections::HashMap;

use super::CollationMetadataProvider;
use crate::exception::Exception;

/// `@internal` `final class CachingCollationMetadataProvider implements CollationMetadataProvider`
pub struct CachingCollationMetadataProvider<'a> {
    /// `private array $cache`: collation => `?non-empty-string`.
    cache: RefCell<HashMap<String, Option<String>>>,
    collation_metadata_provider: Box<dyn CollationMetadataProvider + 'a>,
}

impl<'a> CachingCollationMetadataProvider<'a> {
    /// `__construct(CollationMetadataProvider $collationMetadataProvider)`.
    #[must_use]
    pub fn new(collation_metadata_provider: Box<dyn CollationMetadataProvider + 'a>) -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            collation_metadata_provider,
        }
    }
}

impl CollationMetadataProvider for CachingCollationMetadataProvider<'_> {
    fn get_collation_charset(&self, collation: &str) -> Result<Option<String>, Exception> {
        if let Some(charset) = self.cache.borrow().get(collation) {
            return Ok(charset.clone());
        }

        let charset = self
            .collation_metadata_provider
            .get_collation_charset(collation)?;
        self.cache
            .borrow_mut()
            .insert(collation.to_owned(), charset.clone());

        Ok(charset)
    }
}
