//! Doctrine\DBAL\Platforms\PostgreSQL: the `PostgreSQL` platform's helpers.
//!
//! Upstream: none

mod comparator;
mod postgresql_metadata_provider;

pub use comparator::Comparator;
pub use postgresql_metadata_provider::PostgreSQLMetadataProvider;
pub(crate) use postgresql_metadata_provider::{
    parse_column_type_parameters, parse_default_expression,
};
