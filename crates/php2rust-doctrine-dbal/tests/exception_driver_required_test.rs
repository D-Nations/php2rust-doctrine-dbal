//! Port of tests/Exception/DriverRequiredTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: doctrine/dbal tests/Exception/DriverRequiredTest.php
#![allow(non_snake_case)]

use php2rust_doctrine_dbal::Exception;

#[test]
fn testDriverRequiredWithUrl() {
    let url = "mysql://localhost";
    let exception = Exception::driver_required(Some(url.as_bytes()));

    assert_eq!(
        format!(
            "The options \"driver\" or \"driverClass\" are mandatory if a connection URL without scheme is given to DriverManager::getConnection(). Given URL \"{url}\"."
        ),
        exception.message().to_string_lossy()
    );
}
