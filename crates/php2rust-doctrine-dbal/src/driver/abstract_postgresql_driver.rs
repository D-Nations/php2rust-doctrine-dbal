//! Port of Doctrine\DBAL\Driver\AbstractPostgreSQLDriver.php: the base of
//! the `PostgreSQL` drivers, which picks the platform from the server
//! version. As with [`super::AbstractSQLiteDriver`], the abstract class is
//! a trait whose provided methods are the class's methods; a `PostgreSQL`
//! driver forwards its `Driver` methods to them.
//!
//! The version pattern `/^(?P<major>\d+)(?:\.(?P<minor>\d+)(?:\.(?P<patch>\d+))?)?/`
//! is read without a regex. `version_compare("$major.$minor.$patch",
//! '12.0') >= 0` holds exactly when the major version is at least 12.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/AbstractPostgreSQLDriver.php

use std::sync::Arc;

use super::api::{ExceptionConverter, postgresql};
use super::{BoxFuture, PlatformBox};
use crate::platforms::{PostgreSQL120Platform, PostgreSQLPlatform};
use crate::{Exception, ServerVersionProvider, deprecations};

/// Abstract base implementation of the `Driver` interface for `PostgreSQL`
/// based drivers.
pub trait AbstractPostgreSQLDriver {
    /// `getDatabasePlatform($versionProvider)`.
    fn postgresql_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async move {
            let version = version_provider.get_server_version().await?;
            Ok(Box::new(postgresql_platform_for_version(&version)?) as PlatformBox)
        })
    }

    /// `getExceptionConverter()`.
    fn postgresql_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(postgresql::ExceptionConverter)
    }
}

/// The platform `getDatabasePlatform()` returns for a server version.
pub fn postgresql_platform_for_version(version: &str) -> Result<PostgreSQLPlatform, Exception> {
    let digits = version.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return Err(Exception::invalid_platform_version(
            version,
            "<major_version>.<minor_version>.<patch_version>",
        ));
    }

    // The major version as a number: its digits without leading zeros.
    let major = version[..digits].trim_start_matches('0');
    if major.len() > 2 || (major.len() == 2 && major >= "12") {
        return Ok(PostgreSQL120Platform::new());
    }

    deprecations::trigger("https://github.com/doctrine/dbal/pull/6495", || {
        "Support for Postgres < 12 is deprecated and will be removed in DBAL 5".to_owned()
    });

    Ok(PostgreSQLPlatform::new())
}
