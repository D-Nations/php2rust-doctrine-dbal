//! Port of Doctrine\DBAL\Tools: the DSN parser and, with the `console`
//! feature, the console commands.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: none

#[cfg(feature = "console")]
pub mod console;
mod dsn_parser;

pub use dsn_parser::DsnParser;
