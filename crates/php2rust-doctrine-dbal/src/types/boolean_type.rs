//! Port of Doctrine\DBAL\Types\BooleanType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/BooleanType.php

use php_value::{PhpArray, PhpString, Thrown, Value};

use super::{ConversionError, Type};
use crate::{ParameterType, exception::Exception, platforms::AbstractPlatform};

/// Type that maps an SQL boolean to a PHP boolean.
#[derive(Debug, Default)]
pub struct BooleanType;

impl Type for BooleanType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\BooleanType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_boolean_type_declaration_sql(column)
    }

    fn convert_to_database_value(
        &self,
        value: &Value,
        platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        platform
            .convert_booleans_to_database_value(value)
            .map_err(|e| {
                if e.class() == Exception::UNSUPPORTED {
                    ConversionError::Unsupported(e.message().to_string_lossy().into_owned())
                } else {
                    ConversionError::Thrown(Thrown::new(e.class(), e.message().clone(), 0))
                }
            })
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(platform
            .convert_from_boolean(value)
            .map_or(Value::Null, Value::Bool))
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::Boolean
    }
}
