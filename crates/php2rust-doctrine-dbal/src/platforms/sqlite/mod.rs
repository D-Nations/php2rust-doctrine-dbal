//! Doctrine\DBAL\Platforms\SQLite: the `SQLite` platform's helpers.
//!
//! Upstream: none

mod comparator;

pub use comparator::Comparator;

// Slice SchemaManager.
pub(crate) mod preg;
mod sqlite_metadata_provider;

pub(crate) use sqlite_metadata_provider::{
    FOREIGN_KEY_DETAILS_PATTERN, assertion_error, build_identifier_pattern, nullable_text,
    parse_without_rowid_from_sql, php_int, text as text_value,
};
pub use sqlite_metadata_provider::{ForeignKeyConstraintDetails, SQLiteMetadataProvider};
