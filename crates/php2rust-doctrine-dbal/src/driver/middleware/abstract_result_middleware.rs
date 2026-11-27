//! Port of Doctrine\DBAL\Driver\Middleware\AbstractResultMiddleware.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Middleware/AbstractResultMiddleware.php

use php_value::{PhpArray, PhpString, Value};

use crate::Exception;
use crate::driver::Result;

/// `AbstractResultMiddleware`: a driver result that delegates to the
/// result it wraps.
pub struct AbstractResultMiddleware {
    wrapped_result: Box<dyn Result>,
}

impl AbstractResultMiddleware {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Driver\\Middleware\\AbstractResultMiddleware";

    pub fn new(wrapped_result: Box<dyn Result>) -> Self {
        Self { wrapped_result }
    }
}

impl Result for AbstractResultMiddleware {
    fn fetch_numeric(&mut self) -> std::result::Result<Option<PhpArray>, Exception> {
        self.wrapped_result.fetch_numeric()
    }

    fn fetch_associative(&mut self) -> std::result::Result<Option<PhpArray>, Exception> {
        self.wrapped_result.fetch_associative()
    }

    fn fetch_one(&mut self) -> std::result::Result<Option<Value>, Exception> {
        self.wrapped_result.fetch_one()
    }

    fn fetch_all_numeric(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        self.wrapped_result.fetch_all_numeric()
    }

    fn fetch_all_associative(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        self.wrapped_result.fetch_all_associative()
    }

    fn fetch_first_column(&mut self) -> std::result::Result<Vec<Value>, Exception> {
        self.wrapped_result.fetch_first_column()
    }

    fn row_count(&self) -> std::result::Result<i64, Exception> {
        self.wrapped_result.row_count()
    }

    fn column_count(&self) -> std::result::Result<usize, Exception> {
        self.wrapped_result.column_count()
    }

    /// The wrapped result's `getColumnName()`; a `LogicException` when it
    /// does not have one.
    fn get_column_name(&self, index: i64) -> Option<std::result::Result<PhpString, Exception>> {
        Some(
            self.wrapped_result
                .get_column_name(index)
                .unwrap_or_else(|| {
                    Err(Exception::new(
                        Exception::LOGIC_EXCEPTION,
                        format!(
                            "The driver result {} does not support accessing the column name.",
                            self.wrapped_result.class()
                        ),
                    ))
                }),
        )
    }

    fn free(&mut self) {
        self.wrapped_result.free();
    }

    fn class(&self) -> &str {
        Self::CLASS
    }
}
