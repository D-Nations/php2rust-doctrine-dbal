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

// Slice MySQL, PostgreSQL and Oracle platforms; slice MariaDB platforms.
pub(crate) mod abstract_mysql_platform;
mod mariadb1010_platform;
mod mariadb1052_platform;
mod mariadb1060_platform;
mod mariadb110700_platform;
mod mariadb120300_platform;
pub(crate) mod mariadb_platform;
mod mysql80_platform;
mod mysql84_platform;
mod mysql_platform;
mod oracle_platform;
mod postgresql_platform;
mod sqlserver_platform;

// Slice SQL Server platform.
pub mod sqlserver;

pub use abstract_mysql_platform::{
    LENGTH_LIMIT_BLOB, LENGTH_LIMIT_MEDIUMBLOB, LENGTH_LIMIT_MEDIUMTEXT, LENGTH_LIMIT_TEXT,
    LENGTH_LIMIT_TINYBLOB, LENGTH_LIMIT_TINYTEXT,
};
pub use mariadb_platform::MariaDBPlatform;
pub use mariadb1010_platform::MariaDB1010Platform;
pub use mariadb1052_platform::MariaDB1052Platform;
pub use mariadb1060_platform::MariaDB1060Platform;
pub use mariadb110700_platform::MariaDB110700Platform;
pub use mariadb120300_platform::MariaDB120300Platform;
pub use mysql_platform::MySQLPlatform;
pub use mysql80_platform::MySQL80Platform;
pub use mysql84_platform::MySQL84Platform;
pub use oracle_platform::OraclePlatform;
pub use postgresql_platform::PostgreSQLPlatform;
pub use sqlserver_platform::SQLServerPlatform;
