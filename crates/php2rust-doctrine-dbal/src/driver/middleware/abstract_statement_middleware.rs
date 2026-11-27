//! Port of Doctrine\DBAL\Driver\Middleware\AbstractStatementMiddleware.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Middleware/AbstractStatementMiddleware.php

use php_value::{Key, Value};

use crate::driver::{BoxFuture, Result, Statement};
use crate::{Exception, ParameterType};

/// `AbstractStatementMiddleware`: a driver statement that delegates to the
/// statement it wraps.
pub struct AbstractStatementMiddleware {
    wrapped_statement: Box<dyn Statement>,
}

impl AbstractStatementMiddleware {
    pub const CLASS: &'static str =
        "Doctrine\\DBAL\\Driver\\Middleware\\AbstractStatementMiddleware";

    pub fn new(wrapped_statement: Box<dyn Statement>) -> Self {
        Self { wrapped_statement }
    }
}

impl Statement for AbstractStatementMiddleware {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> std::result::Result<(), Exception> {
        self.wrapped_statement.bind_value(param, value, r#type)
    }

    fn execute(&mut self) -> BoxFuture<'_, std::result::Result<Box<dyn Result>, Exception>> {
        self.wrapped_statement.execute()
    }
}
