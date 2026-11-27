//! Doctrine\DBAL\Portability: the middleware that makes results portable
//! across platforms (empty strings as `NULL`, right-trimmed strings, the
//! case of column names).
//!
//! Upstream: none

mod connection;
mod converter;
mod driver;
mod middleware;
mod optimize_flags;
mod result;
mod statement;

pub use connection::Connection;
pub use converter::Converter;
pub use driver::Driver;
pub use middleware::Middleware;
pub use optimize_flags::OptimizeFlags;
pub use result::Result;
pub use statement::Statement;
