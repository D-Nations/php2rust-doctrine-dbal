//! Port of Doctrine\DBAL\Schema\SequenceEditor.php.
//!
//! A by-value builder. An exception PHP throws from a setter is kept and
//! returned by `create()` (the first one wins).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/SequenceEditor.php

use crate::exception::Exception;

use super::exception;
use super::name::OptionallyQualifiedName;
use super::sequence::Sequence;

#[derive(Debug, Clone)]
pub struct SequenceEditor {
    error: Option<Exception>,
    name: Option<OptionallyQualifiedName>,
    allocation_size: i64,
    initial_value: i64,
    cache_size: Option<i64>,
}

impl Default for SequenceEditor {
    fn default() -> Self {
        Self {
            error: None,
            name: None,
            allocation_size: 1,
            initial_value: 1,
            cache_size: None,
        }
    }
}

impl SequenceEditor {
    /// `@internal` Use `Sequence::editor()` or `Sequence::edit()`.
    pub fn new() -> Self {
        Self::default()
    }

    fn fail(&mut self, e: Exception) {
        self.error.get_or_insert(e);
    }

    #[must_use]
    pub fn set_name(mut self, name: OptionallyQualifiedName) -> Self {
        self.name = Some(name);
        self
    }

    #[must_use]
    pub fn set_unquoted_name(mut self, unqualified_name: &str, qualifier: Option<&str>) -> Self {
        match OptionallyQualifiedName::unquoted(unqualified_name, qualifier) {
            Ok(name) => self.name = Some(name),
            Err(e) => self.fail(e),
        }
        self
    }

    #[must_use]
    pub fn set_quoted_name(mut self, unqualified_name: &str, qualifier: Option<&str>) -> Self {
        match OptionallyQualifiedName::quoted(unqualified_name, qualifier) {
            Ok(name) => self.name = Some(name),
            Err(e) => self.fail(e),
        }
        self
    }

    #[must_use]
    pub fn set_allocation_size(mut self, allocation_size: i64) -> Self {
        self.allocation_size = allocation_size;
        self
    }

    #[must_use]
    pub fn set_initial_value(mut self, initial_value: i64) -> Self {
        self.initial_value = initial_value;
        self
    }

    #[must_use]
    pub fn set_cache_size(mut self, cache_size: Option<i64>) -> Self {
        match cache_size {
            Some(size) if size < 0 => {
                self.fail(exception::sequence_from_negative_cache_size(size));
            }
            _ => self.cache_size = cache_size,
        }
        self
    }

    pub fn create(self) -> Result<Sequence, Exception> {
        if let Some(e) = self.error {
            return Err(e);
        }

        let Some(name) = self.name else {
            return Err(exception::sequence_name_not_set());
        };

        Ok(Sequence::with_options(
            &name.to_string(),
            self.allocation_size,
            self.initial_value,
            self.cache_size,
        ))
    }
}
