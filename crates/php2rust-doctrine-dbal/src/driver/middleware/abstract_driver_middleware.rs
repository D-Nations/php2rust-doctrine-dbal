//! Port of Doctrine\DBAL\Driver\Middleware\AbstractDriverMiddleware.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Middleware/AbstractDriverMiddleware.php

use std::sync::Arc;

use php_value::PhpArray;

use crate::driver::api::ExceptionConverter;
use crate::driver::{BoxFuture, Connection, Driver, PlatformBox};
use crate::{Exception, ServerVersionProvider};

/// `AbstractDriverMiddleware`: a driver that delegates to the driver it
/// wraps.
#[derive(Clone)]
pub struct AbstractDriverMiddleware {
    wrapped_driver: Arc<dyn Driver>,
}

impl AbstractDriverMiddleware {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Driver\\Middleware\\AbstractDriverMiddleware";

    pub fn new(wrapped_driver: Arc<dyn Driver>) -> Self {
        Self { wrapped_driver }
    }
}

impl Driver for AbstractDriverMiddleware {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn connect<'a>(
        &'a self,
        params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn Connection>, Exception>> {
        self.wrapped_driver.connect(params)
    }

    fn get_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        self.wrapped_driver.get_database_platform(version_provider)
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        self.wrapped_driver.get_exception_converter()
    }
}
