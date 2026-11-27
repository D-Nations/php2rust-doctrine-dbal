//! Port of Doctrine\DBAL\Portability\Statement.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/Statement.php

use std::sync::Arc;

use php_value::{Key, Value};

use super::{Converter, Result};
use crate::driver::middleware::AbstractStatementMiddleware;
use crate::driver::{BoxFuture, Result as ResultInterface, Statement as DriverStatement};
use crate::{Exception, ParameterType};

/// `final class Statement extends AbstractStatementMiddleware`: the
/// portability wrapper for a statement.
pub struct Statement {
    parent: AbstractStatementMiddleware,
    converter: Arc<Converter>,
}

impl Statement {
    /// `__construct($stmt, $converter)`: wraps the statement and applies
    /// portability measures.
    pub fn new(stmt: Box<dyn DriverStatement>, converter: Arc<Converter>) -> Self {
        Self {
            parent: AbstractStatementMiddleware::new(stmt),
            converter,
        }
    }
}

impl DriverStatement for Statement {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> std::result::Result<(), Exception> {
        self.parent.bind_value(param, value, r#type)
    }

    fn execute(
        &mut self,
    ) -> BoxFuture<'_, std::result::Result<Box<dyn ResultInterface>, Exception>> {
        Box::pin(async move {
            let result = self.parent.execute().await?;
            Ok(Box::new(Result::new(result, Arc::clone(&self.converter)))
                as Box<dyn ResultInterface>)
        })
    }
}
