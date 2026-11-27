//! Doctrine\DBAL\Platforms\MySQL: the `MySQL` platform's helpers.
//!
//! Upstream: none

mod caching_charset_metadata_provider;
mod caching_collation_metadata_provider;
mod charset_metadata_provider;
mod collation_metadata_provider;
mod comparator;
mod connection_charset_metadata_provider;
mod connection_collation_metadata_provider;
mod default_table_options;
mod mysql_metadata_provider;

pub use caching_charset_metadata_provider::CachingCharsetMetadataProvider;
pub use caching_collation_metadata_provider::CachingCollationMetadataProvider;
pub use charset_metadata_provider::CharsetMetadataProvider;
pub use collation_metadata_provider::CollationMetadataProvider;
pub use comparator::Comparator;
pub use connection_charset_metadata_provider::ConnectionCharsetMetadataProvider;
pub use connection_collation_metadata_provider::ConnectionCollationMetadataProvider;
pub use default_table_options::DefaultTableOptions;
pub use mysql_metadata_provider::MySQLMetadataProvider;
pub(crate) use mysql_metadata_provider::{
    MARIADB_ESCAPE_SEQUENCES, parse_create_options, parse_enum_expression, strtr,
};
