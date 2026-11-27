//! Port of Doctrine\DBAL\Platforms\MySQL\CharsetMetadataProvider\CachingCharsetMetadataProvider.php.
//!
//! `@internal`. The cache is behind a `RefCell` because
//! `CharsetMetadataProvider::get_default_charset_collation()` takes `&self`.
//! A `null` collation is cached like any other answer; an exception from the
//! underlying provider is passed on and nothing is cached, as in PHP.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CharsetMetadataProvider/CachingCharsetMetadataProvider.php

use std::cell::RefCell;
use std::collections::HashMap;

use super::CharsetMetadataProvider;
use crate::exception::Exception;

/// `@internal` `final class CachingCharsetMetadataProvider implements CharsetMetadataProvider`
pub struct CachingCharsetMetadataProvider<'a> {
    /// `private array $cache`: charset => `?non-empty-string`.
    cache: RefCell<HashMap<String, Option<String>>>,
    charset_metadata_provider: Box<dyn CharsetMetadataProvider + 'a>,
}

impl<'a> CachingCharsetMetadataProvider<'a> {
    /// `__construct(CharsetMetadataProvider $charsetMetadataProvider)`.
    #[must_use]
    pub fn new(charset_metadata_provider: Box<dyn CharsetMetadataProvider + 'a>) -> Self {
        Self {
            cache: RefCell::new(HashMap::new()),
            charset_metadata_provider,
        }
    }
}

impl CharsetMetadataProvider for CachingCharsetMetadataProvider<'_> {
    fn get_default_charset_collation(&self, charset: &str) -> Result<Option<String>, Exception> {
        if let Some(collation) = self.cache.borrow().get(charset) {
            return Ok(collation.clone());
        }

        let collation = self
            .charset_metadata_provider
            .get_default_charset_collation(charset)?;
        self.cache
            .borrow_mut()
            .insert(charset.to_owned(), collation.clone());

        Ok(collation)
    }
}
