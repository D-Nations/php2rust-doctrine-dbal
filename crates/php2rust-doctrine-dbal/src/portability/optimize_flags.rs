//! Port of Doctrine\DBAL\Portability\OptimizeFlags.php.
//!
//! PHP tests `$platform instanceof $class` for each platform of its
//! table. The table excludes a flag only for `OraclePlatform`
//! (`PORTABILITY_EMPTY_TO_NULL`), which has no subclass, so the platform's
//! class name decides; the other platforms' masks are 0 and change
//! nothing.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/OptimizeFlags.php

use super::Connection;
use crate::platforms::{
    AbstractPlatform, DB2Platform, OraclePlatform, PostgreSQLPlatform, SQLServerPlatform,
    SQLitePlatform,
};

/// Platform-specific portability flags that need to be excluded from the
/// user-provided mode since the platform already operates in this mode to
/// avoid unnecessary conversion overhead.
const PLATFORMS: [(&str, i64); 5] = [
    (DB2Platform::CLASS, 0),
    (OraclePlatform::CLASS, Connection::PORTABILITY_EMPTY_TO_NULL),
    (PostgreSQLPlatform::CLASS, 0),
    (SQLitePlatform::CLASS, 0),
    (SQLServerPlatform::CLASS, 0),
];

/// `final class OptimizeFlags`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptimizeFlags;

impl OptimizeFlags {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// `__invoke($platform, $flags)`.
    #[must_use]
    pub fn invoke(self, platform: &dyn AbstractPlatform, mut flags: i64) -> i64 {
        for (class, mask) in PLATFORMS {
            if platform.class() == class {
                flags &= !mask;

                break;
            }
        }

        flags
    }
}
