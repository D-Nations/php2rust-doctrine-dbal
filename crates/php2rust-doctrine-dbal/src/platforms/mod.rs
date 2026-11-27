//! Doctrine\DBAL\Platforms: the SQL dialects.
//!
//! Upstream: none

mod abstract_platform;
mod exception;

pub use abstract_platform::AbstractPlatform;
pub use exception::NotSupported;

// Slice SQLite platform.
mod date_interval_unit;
pub mod keywords;
mod no_columns_specified_for_table;
mod platform_state;
mod sqlite_platform;

pub use abstract_platform::AsDynPlatform;
pub use date_interval_unit::{DateIntervalUnit, TrimMode};
pub use no_columns_specified_for_table::{
    NO_COLUMNS_SPECIFIED_FOR_TABLE, PLATFORM_EXCEPTION, no_columns_specified_for_table,
};
pub use platform_state::{CreateTableOptions, PlatformState};
pub use sqlite_platform::SQLitePlatform;

// Slice Comparator.
pub mod mysql;
pub mod sqlite;

// Slice MySQL, PostgreSQL and Oracle platforms.
pub(crate) mod abstract_mysql_platform;
mod mysql80_platform;
mod mysql84_platform;
mod mysql_platform;
mod oracle_platform;
mod postgresql_platform;

pub use abstract_mysql_platform::{
    LENGTH_LIMIT_BLOB, LENGTH_LIMIT_MEDIUMBLOB, LENGTH_LIMIT_MEDIUMTEXT, LENGTH_LIMIT_TEXT,
    LENGTH_LIMIT_TINYBLOB, LENGTH_LIMIT_TINYTEXT,
};
pub use mysql_platform::MySQLPlatform;
pub use mysql80_platform::MySQL80Platform;
pub use mysql84_platform::MySQL84Platform;
pub use oracle_platform::OraclePlatform;
pub use postgresql_platform::PostgreSQLPlatform;
