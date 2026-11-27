//! Port of Doctrine\DBAL\Portability\Result.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/Result.php

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value};

use super::Converter;
use crate::Exception;
use crate::driver::Result as ResultInterface;
use crate::driver::middleware::AbstractResultMiddleware;

/// `final class Result extends AbstractResultMiddleware`. `@internal`: the
/// result can be only instantiated by the portability connection or
/// statement.
pub struct Result {
    parent: AbstractResultMiddleware,
    converter: Arc<Converter>,
}

impl Result {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Portability\\Result";

    /// `__construct($result, $converter)`.
    pub fn new(result: Box<dyn ResultInterface>, converter: Arc<Converter>) -> Self {
        Self {
            parent: AbstractResultMiddleware::new(result),
            converter,
        }
    }
}

impl ResultInterface for Result {
    fn fetch_numeric(&mut self) -> std::result::Result<Option<PhpArray>, Exception> {
        Ok(self.converter.convert_numeric(self.parent.fetch_numeric()?))
    }

    fn fetch_associative(&mut self) -> std::result::Result<Option<PhpArray>, Exception> {
        Ok(self
            .converter
            .convert_associative(self.parent.fetch_associative()?))
    }

    fn fetch_one(&mut self) -> std::result::Result<Option<Value>, Exception> {
        Ok(self.converter.convert_one(self.parent.fetch_one()?))
    }

    fn fetch_all_numeric(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        Ok(self
            .converter
            .convert_all_numeric(self.parent.fetch_all_numeric()?))
    }

    fn fetch_all_associative(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        Ok(self
            .converter
            .convert_all_associative(self.parent.fetch_all_associative()?))
    }

    fn fetch_first_column(&mut self) -> std::result::Result<Vec<Value>, Exception> {
        Ok(self
            .converter
            .convert_first_column(self.parent.fetch_first_column()?))
    }

    fn row_count(&self) -> std::result::Result<i64, Exception> {
        self.parent.row_count()
    }

    fn column_count(&self) -> std::result::Result<usize, Exception> {
        self.parent.column_count()
    }

    fn free(&mut self) {
        self.parent.free();
    }

    fn get_column_name(&self, index: i64) -> Option<std::result::Result<PhpString, Exception>> {
        self.parent
            .get_column_name(index)
            .map(|name| name.map(|name| self.converter.convert_column_name(name)))
    }

    fn class(&self) -> &str {
        Self::CLASS
    }
}
