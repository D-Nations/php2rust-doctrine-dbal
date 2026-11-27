//! Port of
//! Doctrine\DBAL\Platforms\MySQL\CharsetMetadataProvider\CachingCharsetMetadataProvider.php.
//!
//! `@internal`. The cache is a `RefCell`, as the provider is called
//! through `&self`; a failing lookup is not cached (PHP's exception leaves
//! the assignment undone).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CharsetMetadataProvider/CachingCharsetMetadataProvider.php

use std::cell::RefCell;
use std::collections::HashMap;

use super::CharsetMetadataProvider;
use crate::exception::Exception;

/// `@internal`
pub struct CachingCharsetMetadataProvider<'a> {
    charset_metadata_provider: Box<dyn CharsetMetadataProvider + 'a>,
    cache: RefCell<HashMap<String, Option<String>>>,
}

impl<'a> CachingCharsetMetadataProvider<'a> {
    pub fn new(charset_metadata_provider: Box<dyn CharsetMetadataProvider + 'a>) -> Self {
        Self {
            charset_metadata_provider,
            cache: RefCell::new(HashMap::new()),
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
