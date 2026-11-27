//! Doctrine\DBAL\Driver\AbstractOracleDriver: the helpers of the Oracle
//! drivers. The drivers themselves (OCI8, PDO\OCI) are not ported.
//!
//! Upstream: none

mod easy_connect_string;

pub use easy_connect_string::EasyConnectString;
