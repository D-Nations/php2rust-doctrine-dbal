//! Port of Doctrine\DBAL\Cache\QueryCacheProfile.php: the lifetime, key
//! and pool a query's result is cached with.
//!
//! The profile is immutable; the `set*()` methods return a new one, as
//! upstream's do. `generateCacheKeys()` writes the real key from the
//! query, `serialize($params)`, `serialize($types)` (a parameter type
//! enum case serializes as `E:...`, a type object as an object without
//! properties) and the SHA-256 of `serialize($connectionParams)`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Cache/QueryCacheProfile.php

use std::fmt::Write as _;
use std::sync::Arc;

use php_value::object::{EnumCase, GenericObject};
use php_value::serialize::serialize;
use php_value::{Key, PhpArray, PhpString, Value};

use super::pool::CacheItemPool;
use crate::{Exception, ParameterTypes, WrapperParameterType};

/// `Doctrine\DBAL\Cache\QueryCacheProfile`.
#[derive(Debug, Clone, Default)]
pub struct QueryCacheProfile {
    lifetime: i64,
    cache_key: Option<String>,
    result_cache: Option<Arc<dyn CacheItemPool>>,
}

impl PartialEq for QueryCacheProfile {
    fn eq(&self, other: &Self) -> bool {
        self.lifetime == other.lifetime
            && self.cache_key == other.cache_key
            && match (&self.result_cache, &other.result_cache) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

/// `serialize($types)`.
fn serialize_types(types: &ParameterTypes) -> PhpString {
    let mut array = PhpArray::new();
    for (key, type_) in types {
        let value = match type_ {
            WrapperParameterType::Parameter(parameter) => Value::Object(
                EnumCase::new("Doctrine\\DBAL\\ParameterType", parameter.name(), None).object(),
            ),
            WrapperParameterType::Array(parameter) => Value::Object(
                EnumCase::new("Doctrine\\DBAL\\ArrayParameterType", parameter.name(), None)
                    .object(),
            ),
            WrapperParameterType::Name(name) => Value::String(name.clone()),
            WrapperParameterType::Type(db_type) => Value::Object(Arc::new(GenericObject::new(
                db_type.class(),
                PhpArray::new(),
            ))),
        };
        array.insert(key.clone(), value);
    }
    serialize(&Value::Array(array))
}

fn hex(digest: &[u8]) -> String {
    digest.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

impl QueryCacheProfile {
    /// `new QueryCacheProfile($lifetime, $cacheKey, $resultCache)`.
    pub fn new(
        lifetime: i64,
        cache_key: Option<String>,
        result_cache: Option<Arc<dyn CacheItemPool>>,
    ) -> Self {
        Self {
            lifetime,
            cache_key,
            result_cache,
        }
    }

    pub fn get_result_cache(&self) -> Option<&Arc<dyn CacheItemPool>> {
        self.result_cache.as_ref()
    }

    pub fn get_lifetime(&self) -> i64 {
        self.lifetime
    }

    /// `getCacheKey()`: throws `NoCacheKey` when no key was set.
    pub fn get_cache_key(&self) -> Result<&str, Exception> {
        self.cache_key
            .as_deref()
            .ok_or_else(Exception::no_cache_key)
    }

    /// Generates the real cache key from query, params, types and
    /// connection parameters: `[$cacheKey, $realCacheKey]`.
    pub fn generate_cache_keys(
        &self,
        sql: &str,
        params: &PhpArray,
        types: &ParameterTypes,
        connection_params: &PhpArray,
    ) -> (String, PhpString) {
        let mut connection_params = connection_params.clone();
        connection_params.remove(&Key::from("password"));

        let mut real_cache_key = b"query=".to_vec();
        real_cache_key.extend_from_slice(sql.as_bytes());
        real_cache_key.extend_from_slice(b"&params=");
        real_cache_key.extend_from_slice(serialize(&Value::Array(params.clone())).as_bytes());
        real_cache_key.extend_from_slice(b"&types=");
        real_cache_key.extend_from_slice(serialize_types(types).as_bytes());
        real_cache_key.extend_from_slice(b"&connectionParams=");
        {
            use sha2::Digest as _;
            let digest =
                sha2::Sha256::digest(serialize(&Value::Array(connection_params)).as_bytes());
            real_cache_key.extend_from_slice(hex(&digest).as_bytes());
        }
        // should the key be automatically generated using the inputs or is
        // the cache key set?
        let cache_key = self.cache_key.clone().unwrap_or_else(|| {
            use sha1::Digest as _;
            hex(&sha1::Sha1::digest(&real_cache_key))
        });

        (cache_key, PhpString::from(real_cache_key))
    }

    #[must_use]
    pub fn set_result_cache(&self, cache: Arc<dyn CacheItemPool>) -> Self {
        Self::new(self.lifetime, self.cache_key.clone(), Some(cache))
    }

    #[must_use]
    pub fn set_cache_key(&self, cache_key: Option<String>) -> Self {
        Self::new(self.lifetime, cache_key, self.result_cache.clone())
    }

    #[must_use]
    pub fn set_lifetime(&self, lifetime: i64) -> Self {
        Self::new(lifetime, self.cache_key.clone(), self.result_cache.clone())
    }
}
