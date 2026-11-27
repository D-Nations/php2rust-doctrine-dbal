//! The `Driver\Middleware` namespace: the abstract middlewares that wrap a
//! driver, its connections, statements and results, delegating each method
//! to the wrapped object.
//!
//! PHP's middlewares extend these abstract classes and override some
//! methods. A Rust middleware holds one of these structs and implements the
//! trait itself, delegating the methods it does not change to it; the
//! structs implement the traits too, as an anonymous subclass that
//! overrides nothing does.
//!
//! [`Middleware`] is the `Driver\Middleware` interface itself
//! (src/Driver/Middleware.php): it wraps a driver in another one.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Middleware.php
//! Upstream: doctrine/dbal src/Driver/Middleware/AbstractConnectionMiddleware.php src/Driver/Middleware/AbstractDriverMiddleware.php src/Driver/Middleware/AbstractResultMiddleware.php src/Driver/Middleware/AbstractStatementMiddleware.php

mod abstract_connection_middleware;
mod abstract_driver_middleware;
mod abstract_result_middleware;
mod abstract_statement_middleware;

pub use abstract_connection_middleware::AbstractConnectionMiddleware;
pub use abstract_driver_middleware::AbstractDriverMiddleware;
pub use abstract_result_middleware::AbstractResultMiddleware;
pub use abstract_statement_middleware::AbstractStatementMiddleware;

use std::sync::Arc;

use super::Driver;

/// `interface Middleware`: wraps a driver.
pub trait Middleware: Send + Sync {
    /// `wrap(Driver $driver): Driver`.
    fn wrap(&self, driver: Arc<dyn Driver>) -> Arc<dyn Driver>;
}
