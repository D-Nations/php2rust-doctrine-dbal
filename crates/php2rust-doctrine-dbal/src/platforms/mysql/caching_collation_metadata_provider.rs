//! Port of
//! Doctrine\DBAL\Platforms\MySQL\CollationMetadataProvider\CachingCollationMetadataProvider.php.
//!
//! `@internal`. The cache is a `RefCell`, as the provider is called
//! through `&self`; a failing lookup is not cached (PHP's exception leaves
//! the assignment undone).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/CollationMetadataProvider/CachingCollationMetadataProvider.php

use std::cell::RefCell;
use std::collections::HashMap;

use super::CollationMetadataProvider;
use crate::exception::Exception;

/// `@internal`
pub struct CachingCollationMetadataProvider<'a> {
    collation_metadata_provider: Box<dyn CollationMetadataProvider + 'a>,
    cache: RefCell<HashMap<String, Option<String>>>,
}

impl<'a> CachingCollationMetadataProvider<'a> {
    pub fn new(collation_metadata_provider: Box<dyn CollationMetadataProvider + 'a>) -> Self {
        Self {
            collation_metadata_provider,
            cache: RefCell::new(HashMap::new()),
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
