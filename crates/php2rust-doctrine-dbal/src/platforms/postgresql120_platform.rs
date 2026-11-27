//! Port of Doctrine\DBAL\Platforms\PostgreSQL120Platform.php (deprecated
//! upstream, removed once support for Postgres < 12 is dropped): the
//! `PostgreSQL` 12.0 platform.
//!
//! PHP's subclass only overrides `getDefaultColumnValueSQLSnippet()`, so
//! it is a [`PostgreSQLPlatform`] made by [`PostgreSQL120Platform::new()`]:
//! its `class()` is `PostgreSQL120Platform` and it reads the default of a
//! generated column as `NULL`. `instanceof PostgreSQLPlatform` holds for
//! both, as in PHP.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/PostgreSQL120Platform.php

use super::PostgreSQLPlatform;

/// `getDefaultColumnValueSQLSnippet()`: in case of a `GENERATED ALWAYS AS
/// (foobar) STORED` column (added in `PostgreSQL` 12.0), `pg_get_expr(adbin,
/// adrelid)` returns the 'foobar' part, which is not the 'default' value
/// of the column but its 'definition', so in that case it is forced to
/// NULL as DBAL uses that column only for the 'default' value.
pub(super) const DEFAULT_COLUMN_VALUE_SQL_SNIPPET: &str = "    SELECT
        CASE
            WHEN a.attgenerated = 's' THEN NULL
            ELSE pg_get_expr(adbin, adrelid)
        END
     FROM pg_attrdef
     WHERE c.oid = pg_attrdef.adrelid
        AND pg_attrdef.adnum=a.attnum";

/// Provides the behavior, features and SQL dialect of the `PostgreSQL` 12.0
/// database platform.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostgreSQL120Platform;

impl PostgreSQL120Platform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\PostgreSQL120Platform";

    /// `new PostgreSQL120Platform()`.
    #[allow(
        clippy::new_ret_no_self,
        reason = "PHP's subclass is the PostgreSQLPlatform type with the 12.0 flag"
    )]
    pub fn new() -> PostgreSQLPlatform {
        PostgreSQLPlatform::new_120()
    }
}
