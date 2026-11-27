//! Port of Doctrine\DBAL\Portability\Middleware.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/Middleware.php

use std::sync::Arc;

use super::Driver;
use crate::ColumnCase;
use crate::driver::Driver as DriverInterface;
use crate::driver::middleware::Middleware as MiddlewareInterface;

/// `final class Middleware implements Middleware`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Middleware {
    mode: i64,
    case: Option<ColumnCase>,
}

impl Middleware {
    /// `__construct($mode, $case)`.
    #[must_use]
    pub fn new(mode: i64, case: Option<ColumnCase>) -> Self {
        Self { mode, case }
    }
}

impl MiddlewareInterface for Middleware {
    fn wrap(&self, driver: Arc<dyn DriverInterface>) -> Arc<dyn DriverInterface> {
        if self.mode != 0 {
            return Arc::new(Driver::new(driver, self.mode, self.case));
        }

        driver
    }
}
