//! Port of Doctrine\DBAL\Cache\CacheException.php and its subclasses
//! Cache\Exception\NoCacheKey.php and
//! Cache\Exception\NoResultDriverConfigured.php, as classes of
//! [`Exception`].
//!
//! The PSR-6 pool's `InvalidArgumentException` for an invalid key passes
//! through as the pool's class.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Cache/CacheException.php src/Cache/Exception/NoCacheKey.php
//! Upstream: doctrine/dbal src/Cache/Exception/NoResultDriverConfigured.php

use php_value::Thrown;

use super::pool::PSR_INVALID_ARGUMENT_EXCEPTION;
use crate::Exception;

impl Exception {
    /// `Doctrine\DBAL\Cache\CacheException`.
    pub const CACHE_EXCEPTION: &'static str = "Doctrine\\DBAL\\Cache\\CacheException";
    pub const NO_CACHE_KEY: &'static str = "Doctrine\\DBAL\\Cache\\Exception\\NoCacheKey";
    pub const NO_RESULT_DRIVER_CONFIGURED: &'static str =
        "Doctrine\\DBAL\\Cache\\Exception\\NoResultDriverConfigured";
    /// The PSR-6 pool's exception for an invalid key.
    pub const PSR_CACHE_INVALID_ARGUMENT_EXCEPTION: &'static str = PSR_INVALID_ARGUMENT_EXCEPTION;

    /// `NoCacheKey::new()`.
    pub fn no_cache_key() -> Self {
        Self::new(Self::NO_CACHE_KEY, "No cache key was set.")
    }

    /// `NoResultDriverConfigured::new()`.
    pub fn no_result_driver_configured() -> Self {
        Self::new(
            Self::NO_RESULT_DRIVER_CONFIGURED,
            "Trying to cache a query but no result driver is configured.",
        )
    }

    /// An exception the pool threw, as it reaches the caller.
    pub fn from_cache_pool(thrown: Thrown) -> Self {
        let class = if thrown.class == PSR_INVALID_ARGUMENT_EXCEPTION {
            Self::PSR_CACHE_INVALID_ARGUMENT_EXCEPTION
        } else {
            Self::UNSUPPORTED
        };
        Self::new(class, thrown.message).with_code(thrown.code)
    }
}

/// The parents of the classes above, for `instanceof`.
pub(crate) fn parents(class: &str) -> &'static [&'static str] {
    match class {
        Exception::CACHE_EXCEPTION => &["Doctrine\\DBAL\\Exception", "Exception"],
        Exception::NO_CACHE_KEY | Exception::NO_RESULT_DRIVER_CONFIGURED => {
            &[Exception::CACHE_EXCEPTION]
        }
        PSR_INVALID_ARGUMENT_EXCEPTION => &[
            "Psr\\Cache\\InvalidArgumentException",
            "InvalidArgumentException",
        ],
        _ => &[],
    }
}
