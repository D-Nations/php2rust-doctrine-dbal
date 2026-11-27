//! A PSR-6 cache item pool, in memory, where upstream takes a
//! `Psr\Cache\CacheItemPoolInterface` (the result cache of
//! `QueryCacheProfile` and the `Configuration`; upstream's tests use
//! `Symfony\Component\Cache\Adapter\ArrayAdapter`). The ORM's caches use
//! the same pool. It may later move to a psr-cache crate of its own.
//!
//! `ArrayAdapter` keeps each value as a copy (it stores serialized
//! values by default), checks the keys as `CacheItem::validateKey()`
//! does, expires items by `microtime(true)`, and `clear($prefix)` drops
//! the items whose key starts with the prefix, and the expired ones.
//!
//! A pool value is a PHP value, the result cache's array of
//! [`CachedResult`]s, or another object (the ORM's cache entries and
//! parsed queries), which [`PoolValue::downcast_ref`] gives back.
//!
//! Upstream: none

use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{Key, Thrown, Value};

use super::ArrayResult;

/// The characters a PSR-6 key must not contain.
pub const RESERVED_CHARACTERS: &str = "{}()/\\@:";

/// The class of the exception an invalid key throws
/// (`Psr\Cache\InvalidArgumentException`, as Symfony's adapters throw it).
pub const PSR_INVALID_ARGUMENT_EXCEPTION: &str =
    "Symfony\\Component\\Cache\\Exception\\InvalidArgumentException";

/// An object a pool holds. Two are equal when their types are and
/// `PartialEq` says so.
pub trait PoolObject: Any + Debug + Send + Sync {
    fn as_any(&self) -> &dyn Any;

    fn eq_object(&self, other: &dyn PoolObject) -> bool;
}

impl<T: Any + Debug + Send + Sync + PartialEq> PoolObject for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn eq_object(&self, other: &dyn PoolObject) -> bool {
        other.as_any().downcast_ref::<T>() == Some(self)
    }
}

/// One entry of the result cache's array: an `ArrayResult`, or another
/// value (the rows of an older format, which the connection does not
/// read).
#[derive(Debug, Clone, PartialEq)]
pub enum CachedResult {
    ArrayResult(ArrayResult),
    Value(Value),
}

/// What a pool item holds.
#[derive(Debug, Clone)]
pub enum PoolValue {
    /// A PHP value.
    Value(Value),
    /// `Connection::executeCacheQuery()`'s array: the results by their
    /// real cache key, in insertion order.
    Results(Vec<(Key, CachedResult)>),
    /// Another object.
    Object(Arc<dyn PoolObject>),
}

impl PartialEq for PoolValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value(a), Self::Value(b)) => a == b,
            (Self::Results(a), Self::Results(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a.eq_object(b.as_ref()),
            _ => false,
        }
    }
}

impl PoolValue {
    /// An object value.
    pub fn object<T: PoolObject>(object: T) -> Self {
        Self::Object(Arc::new(object))
    }

    /// The object, if the value is one of type `T`.
    pub fn downcast_ref<T: PoolObject>(&self) -> Option<&T> {
        match self {
            Self::Object(object) => object.as_any().downcast_ref::<T>(),
            _ => None,
        }
    }

    /// `count($value)` for an array value, as a test counts one.
    pub fn count(&self) -> Option<usize> {
        match self {
            Self::Value(Value::Array(array)) => Some(array.len()),
            Self::Results(results) => Some(results.len()),
            _ => None,
        }
    }
}

/// `Psr\Cache\CacheItemInterface`.
#[derive(Debug, Clone, PartialEq)]
pub struct CacheItem {
    key: String,
    value: Option<PoolValue>,
    is_hit: bool,
    /// `expiresAfter()`, in seconds.
    lifetime: Option<i64>,
}

impl CacheItem {
    /// An item as a pool gives it: its key, and its value on a hit.
    pub fn new(key: impl Into<String>, value: Option<PoolValue>) -> Self {
        Self {
            key: key.into(),
            is_hit: value.is_some(),
            value,
            lifetime: None,
        }
    }

    pub fn get_key(&self) -> &str {
        &self.key
    }

    pub fn is_hit(&self) -> bool {
        self.is_hit
    }

    /// `get()`: the value, or `None` for a miss.
    pub fn get(&self) -> Option<&PoolValue> {
        self.value.as_ref()
    }

    /// `set($value)`.
    #[must_use]
    pub fn set(mut self, value: PoolValue) -> Self {
        self.value = Some(value);
        self
    }

    /// `expiresAfter($seconds)`.
    #[must_use]
    pub fn expires_after(mut self, seconds: i64) -> Self {
        self.lifetime = Some(seconds);
        self
    }

    /// The value `save()` stores and its `expiresAfter()` lifetime.
    pub fn into_parts(self) -> (String, Option<PoolValue>, Option<i64>) {
        (self.key, self.value, self.lifetime)
    }
}

/// `Psr\Cache\CacheItemPoolInterface`, as far as DBAL and the ORM use it.
pub trait CacheItemPool: Debug + Send + Sync {
    fn get_item(&self, key: &str) -> Result<CacheItem, Thrown>;

    /// `getItems($keys)`: the items under their keys, in order.
    fn get_items(&self, keys: &[String]) -> Result<Vec<CacheItem>, Thrown>;

    fn has_item(&self, key: &str) -> Result<bool, Thrown>;

    /// `clear($prefix)` (Symfony's `AdapterInterface::clear()`).
    fn clear(&self, prefix: &str) -> bool;

    fn delete_item(&self, key: &str) -> Result<bool, Thrown>;

    fn save(&self, item: CacheItem) -> bool;
}

/// `CacheItem::validateKey()`.
pub fn validate_key(key: &str) -> Result<(), Thrown> {
    if key.is_empty() {
        return Err(Thrown::new(
            PSR_INVALID_ARGUMENT_EXCEPTION,
            "Cache key length must be greater than zero.",
            0,
        ));
    }
    if key.contains(|c| RESERVED_CHARACTERS.contains(c)) {
        return Err(Thrown::new(
            PSR_INVALID_ARGUMENT_EXCEPTION,
            format!("Cache key \"{key}\" contains reserved characters \"{RESERVED_CHARACTERS}\"."),
            0,
        ));
    }
    Ok(())
}

/// `Symfony\Component\Cache\Adapter\ArrayAdapter`.
#[derive(Debug)]
pub struct ArrayAdapter {
    default_lifetime: i64,
    /// `$storeSerialized`. Values are copies either way; a value read back
    /// is the value saved.
    store_serialized: bool,
    /// Each key's value and expiry (`microtime(true)`; `None` never).
    values: Mutex<HashMap<String, (PoolValue, Option<f64>)>>,
}

impl Default for ArrayAdapter {
    fn default() -> Self {
        Self::with_options(0, true)
    }
}

impl ArrayAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// `new ArrayAdapter($defaultLifetime)`.
    pub fn with_default_lifetime(default_lifetime: i64) -> Self {
        Self::with_options(default_lifetime, true)
    }

    /// `new ArrayAdapter($defaultLifetime, $storeSerialized)`.
    pub fn with_options(default_lifetime: i64, store_serialized: bool) -> Self {
        Self {
            default_lifetime,
            store_serialized,
            values: Mutex::default(),
        }
    }

    /// Whether the adapter stores serialized values.
    pub fn stores_serialized(&self) -> bool {
        self.store_serialized
    }

    /// `getValues()`: the stored values, by key.
    pub fn get_values(&self) -> Vec<(String, PoolValue)> {
        self.values()
            .iter()
            .map(|(key, (value, _))| (key.clone(), value.clone()))
            .collect()
    }

    fn values(&self) -> MutexGuard<'_, HashMap<String, (PoolValue, Option<f64>)>> {
        self.values.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn live(expiry: Option<f64>, now: f64) -> bool {
        expiry.is_none_or(|expiry| expiry > now)
    }

    fn item(&self, key: &str) -> CacheItem {
        let now = php_value::clock::microtime();
        let value = self
            .values()
            .get(key)
            .filter(|(_, expiry)| Self::live(*expiry, now))
            .map(|(value, _)| value.clone());
        CacheItem::new(key, value)
    }
}

impl CacheItemPool for ArrayAdapter {
    fn get_item(&self, key: &str) -> Result<CacheItem, Thrown> {
        validate_key(key)?;
        Ok(self.item(key))
    }

    fn get_items(&self, keys: &[String]) -> Result<Vec<CacheItem>, Thrown> {
        keys.iter().map(|key| self.get_item(key)).collect()
    }

    fn has_item(&self, key: &str) -> Result<bool, Thrown> {
        validate_key(key)?;
        Ok(self.item(key).is_hit)
    }

    fn clear(&self, prefix: &str) -> bool {
        let mut values = self.values();
        if prefix.is_empty() {
            values.clear();
        } else {
            let now = php_value::clock::microtime();
            values.retain(|key, (_, expiry)| Self::live(*expiry, now) && !key.starts_with(prefix));
        }
        true
    }

    fn delete_item(&self, key: &str) -> Result<bool, Thrown> {
        validate_key(key)?;
        self.values().remove(key);
        Ok(true)
    }

    fn save(&self, item: CacheItem) -> bool {
        let (key, value, item_lifetime) = item.into_parts();
        let Some(value) = value else {
            self.values().remove(&key);
            return true;
        };
        let lifetime = item_lifetime.unwrap_or(self.default_lifetime);
        let expiry = if item_lifetime.is_some() || lifetime > 0 {
            #[expect(clippy::cast_precision_loss, reason = "the expiry is a float")]
            Some(php_value::clock::microtime() + lifetime as f64)
        } else {
            None
        };
        if expiry.is_some_and(|expiry| expiry <= php_value::clock::microtime()) {
            self.values().remove(&key);
            return true;
        }
        self.values().insert(key, (value, expiry));
        true
    }
}
