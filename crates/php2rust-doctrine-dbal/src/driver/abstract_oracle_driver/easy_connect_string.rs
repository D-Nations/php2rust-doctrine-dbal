//! Port of Doctrine\DBAL\Driver\AbstractOracleDriver\EasyConnectString.php:
//! an Oracle Easy Connect string
//! (<https://docs.oracle.com/database/121/NETAG/naming.htm>).
//!
//! A value is rendered with `(string) $value`; an object that is not
//! `Stringable` fails as PHP's cast does, with an `Error`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/AbstractOracleDriver/EasyConnectString.php

use std::fmt;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::{Exception, deprecations};

/// `final class EasyConnectString`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EasyConnectString {
    string: PhpString,
}

/// `isset($params[$key])`.
fn isset<'a>(params: &'a PhpArray, key: &str) -> Option<&'a Value> {
    params.get(&Key::from(key)).filter(|value| !value.is_null())
}

impl EasyConnectString {
    fn new(string: PhpString) -> Self {
        Self { string }
    }

    /// `__toString()`.
    pub fn to_php_string(&self) -> &PhpString {
        &self.string
    }

    /// `fromArray($params)`: creates the object from an array
    /// representation.
    pub fn from_array(params: &PhpArray) -> Result<Self, Exception> {
        Ok(Self::new(render_params(params)?))
    }

    /// `fromConnectionParameters($params)`: creates the object from the
    /// given DBAL connection parameters.
    pub fn from_connection_parameters(params: &PhpArray) -> Result<Self, Exception> {
        if let Some(connect_string) = isset(params, "connectstring") {
            return Ok(Self::new(cast(connect_string)?));
        }

        let Some(host) = isset(params, "host") else {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/7244", || {
                "Not specifying either of the \"host\" and \"connectstring\" parameters is deprecated."
                    .to_owned()
            });

            return Ok(Self::new(match isset(params, "dbname") {
                Some(dbname) => cast(dbname)?,
                None => PhpString::default(),
            }));
        };

        let mut connect_data = PhpArray::new();

        if isset(params, "service").is_some() {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/7042", || {
                "Using the \"service\" parameter to indicate that the value of the \"dbname\" parameter is the service name is deprecated. Use the \"servicename\" parameter instead."
                    .to_owned()
            });
        }

        if isset(params, "dbname").is_some() {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/7239", || {
                "Using the \"dbname\" parameter is deprecated. Use \"servicename\", \"sid\" or \"connectstring\" instead."
                    .to_owned()
            });
        }

        if let Some(service_name) = isset(params, "servicename").or_else(|| isset(params, "dbname"))
        {
            let mut service_key = "SID";

            if isset(params, "service").is_some() || isset(params, "servicename").is_some() {
                service_key = "SERVICE_NAME";
            }

            connect_data.insert(service_key, service_name.clone());
        }

        if let Some(sid) = isset(params, "sid") {
            connect_data.insert("SID", sid.clone());
        }

        if let Some(instance_name) = isset(params, "instancename") {
            connect_data.insert("INSTANCE_NAME", instance_name.clone());
        }

        if params
            .get(&Key::from("pooled"))
            .is_some_and(Value::is_truthy)
        {
            connect_data.insert("SERVER", Value::string("POOLED"));
        }

        let protocol = match params.get(&Key::from("driverOptions")) {
            Some(Value::Array(driver_options)) => isset(driver_options, "protocol").cloned(),
            _ => None,
        };

        let mut address = PhpArray::new();
        address.insert("PROTOCOL", protocol.unwrap_or_else(|| Value::string("TCP")));
        address.insert("HOST", host.clone());
        address.insert(
            "PORT",
            isset(params, "port").cloned().unwrap_or(Value::Int(1521)),
        );

        let mut description = PhpArray::new();
        description.insert("ADDRESS", Value::Array(address));
        description.insert("CONNECT_DATA", Value::Array(connect_data));

        let mut array = PhpArray::new();
        array.insert("DESCRIPTION", Value::Array(description));

        Self::from_array(&array)
    }
}

impl fmt::Display for EasyConnectString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(self.string.as_bytes()))
    }
}

/// `(string) $value`.
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

/// `renderParams($params)`.
fn render_params(params: &PhpArray) -> Result<PhpString, Exception> {
    let mut chunks = Vec::new();

    for (key, value) in params.iter() {
        let string = render_value(value)?;

        if !string.is_empty() {
            chunks.push(b'(');
            match key {
                Key::Str(name) => chunks.extend_from_slice(name.as_bytes()),
                Key::Int(index) => chunks.extend_from_slice(index.to_string().as_bytes()),
            }
            chunks.push(b'=');
            chunks.extend_from_slice(string.as_bytes());
            chunks.push(b')');
        }
    }

    Ok(PhpString::from(chunks))
}

/// `renderValue($value)`.
fn render_value(value: &Value) -> Result<PhpString, Exception> {
    if let Value::Array(array) = value {
        return render_params(array);
    }

    cast(value)
}
