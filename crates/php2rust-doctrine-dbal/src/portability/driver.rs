//! Port of Doctrine\DBAL\Portability\Driver.php.
//!
//! PHP passes the driver connection, a `ServerVersionProvider`, to
//! `getDatabasePlatform()`; here an adapter gives the connection's
//! `getServerVersion()`. PHP leaves the case of column names to PDO
//! (`PDO::ATTR_CASE`) when the native connection is a `PDO` object. No
//! native connection here is one (the `pdo_sqlite` driver is on sqlx), so
//! the converter changes the case, which gives the same column names.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/Driver.php

use std::sync::Arc;

use php_value::PhpArray;

use super::{Connection, Converter, OptimizeFlags};
use crate::driver::api::ExceptionConverter;
use crate::driver::middleware::AbstractDriverMiddleware;
use crate::driver::{
    BoxFuture, Connection as ConnectionInterface, Driver as DriverInterface, PlatformBox,
};
use crate::{ColumnCase, Exception, ServerVersionProvider};

/// `final class Driver extends AbstractDriverMiddleware`.
pub struct Driver {
    parent: AbstractDriverMiddleware,
    mode: i64,
    case: Option<ColumnCase>,
}

impl Driver {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Portability\\Driver";

    /// `__construct($driver, $mode, $case)`.
    pub fn new(driver: Arc<dyn DriverInterface>, mode: i64, case: Option<ColumnCase>) -> Self {
        Self {
            parent: AbstractDriverMiddleware::new(driver),
            mode,
            case,
        }
    }
}

/// The driver connection as the `ServerVersionProvider` it is in PHP.
struct ConnectionVersion<'a>(&'a dyn ConnectionInterface);

impl ServerVersionProvider for ConnectionVersion<'_> {
    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        self.0.get_server_version()
    }
}

impl DriverInterface for Driver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn connect<'a>(
        &'a self,
        params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn ConnectionInterface>, Exception>> {
        Box::pin(async move {
            let connection = self.parent.connect(params).await?;

            let platform = self
                .get_database_platform(&ConnectionVersion(connection.as_ref()))
                .await?;
            let portability = OptimizeFlags::new().invoke(platform.as_dyn(), self.mode);

            let mut case = None;

            if let Some(column_case) = self.case
                && (portability & Connection::PORTABILITY_FIX_CASE) != 0
            {
                case = Some(match column_case {
                    ColumnCase::Lower => Converter::CASE_LOWER,
                    ColumnCase::Upper => Converter::CASE_UPPER,
                });
            }

            let convert_empty_string_to_null =
                (portability & Connection::PORTABILITY_EMPTY_TO_NULL) != 0;
            let right_trim_string = (portability & Connection::PORTABILITY_RTRIM) != 0;

            if !convert_empty_string_to_null && !right_trim_string && case.is_none() {
                return Ok(connection);
            }

            Ok(Arc::new(Connection::new(
                connection,
                Arc::new(Converter::new(
                    convert_empty_string_to_null,
                    right_trim_string,
                    case,
                )?),
            )) as Arc<dyn ConnectionInterface>)
        })
    }

    fn get_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        self.parent.get_database_platform(version_provider)
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        self.parent.get_exception_converter()
    }
}
