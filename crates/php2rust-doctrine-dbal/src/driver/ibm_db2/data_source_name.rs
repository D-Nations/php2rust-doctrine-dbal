//! Port of Doctrine\DBAL\Driver\IBMDB2\DataSourceName.php: a Db2 DSN.
//!
//! `sprintf('%s', $value)` gives `(string) $value` (`Array` for an
//! array); an object that is not `Stringable` fails with an `Error`, as
//! in PHP.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/IBMDB2/DataSourceName.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::Exception;

/// `final class DataSourceName`.
#[derive(Clone, PartialEq, Eq)]
pub struct DataSourceName {
    string: PhpString,
}

/// `#[SensitiveParameter]`: the DSN holds the password, so `Debug` does
/// not show it.
impl std::fmt::Debug for DataSourceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSourceName").finish_non_exhaustive()
    }
}

/// `isset($params[$key])`.
fn isset<'a>(params: &'a PhpArray, key: &str) -> Option<&'a Value> {
    params.get(&Key::from(key)).filter(|value| !value.is_null())
}

/// `sprintf('%s', $value)`.
fn cast(value: &Value) -> Result<PhpString, Exception> {
    if let Value::Array(_) = value {
        return Ok(PhpString::from("Array"));
    }

    value.to_php_string_or_stringable().ok_or_else(|| {
        Exception::new(
            "Error",
            format!(
                "Object of class {} could not be converted to string",
                match value {
                    Value::Object(object) => object.class().to_owned(),
                    _ => String::new(),
                }
            ),
        )
    })
}

impl DataSourceName {
    /// `toString()`.
    pub fn to_php_string(&self) -> &PhpString {
        &self.string
    }

    /// `fromArray($params)`: creates the object from an array
    /// representation.
    pub fn from_array(params: &PhpArray) -> Result<Self, Exception> {
        let mut chunks: Vec<Vec<u8>> = Vec::new();

        for (key, value) in params.iter() {
            let mut chunk = match key {
                Key::Str(name) => name.as_bytes().to_vec(),
                Key::Int(index) => index.to_string().into_bytes(),
            };
            chunk.push(b'=');
            chunk.extend_from_slice(cast(value)?.as_bytes());
            chunks.push(chunk);
        }

        Ok(Self {
            string: PhpString::from(chunks.join(&b';')),
        })
    }

    /// `fromConnectionParameters($params)`: creates the object from the
    /// given DBAL connection parameters.
    pub fn from_connection_parameters(params: &PhpArray) -> Result<Self, Exception> {
        if let Some(dbname) = isset(params, "dbname") {
            let dbname = cast(dbname)?;
            if dbname.as_bytes().contains(&b'=') {
                return Ok(Self { string: dbname });
            }
        }

        let mut dsn_params = PhpArray::new();

        for (dbal_param, dsn_param) in [
            ("host", "HOSTNAME"),
            ("port", "PORT"),
            ("protocol", "PROTOCOL"),
            ("dbname", "DATABASE"),
            ("user", "UID"),
            ("password", "PWD"),
        ] {
            if let Some(value) = isset(params, dbal_param) {
                dsn_params.insert(dsn_param, value.clone());
            }
        }

        Self::from_array(&dsn_params)
    }
}
