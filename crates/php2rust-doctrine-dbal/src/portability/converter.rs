//! Port of Doctrine\DBAL\Portability\Converter.php.
//!
//! PHP builds the conversions as closures once, in the constructor; here
//! each method applies the same steps in the same order: an empty string
//! becomes `NULL`, then a string is right-trimmed (`rtrim()`'s
//! characters: space, tab, newline, carriage return, NUL and vertical
//! tab), then the keys of an associative row change case
//! (`array_change_key_case()`, ASCII as `strtolower()`/`strtoupper()`).
//! PHP's `false` for "no row" is `None`, which every conversion keeps.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Portability/Converter.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::Exception;

/// `final class Converter`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converter {
    convert_empty_string_to_null: bool,
    right_trim_string: bool,
    case: Option<i64>,
}

impl Converter {
    /// `CASE_LOWER`.
    pub const CASE_LOWER: i64 = 0;
    /// `CASE_UPPER`.
    pub const CASE_UPPER: i64 = 1;

    /// `__construct($convertEmptyStringToNull, $rightTrimString, $case)`.
    /// A `$case` other than `CASE_LOWER`, `CASE_UPPER` and `null` fails
    /// the constructor's `match` with an `UnhandledMatchError`.
    pub fn new(
        convert_empty_string_to_null: bool,
        right_trim_string: bool,
        case: Option<i64>,
    ) -> Result<Self, Exception> {
        if let Some(case) = case
            && case != Self::CASE_LOWER
            && case != Self::CASE_UPPER
        {
            return Err(Exception::new(
                "UnhandledMatchError",
                format!("Unhandled match case {case}"),
            ));
        }

        Ok(Self {
            convert_empty_string_to_null,
            right_trim_string,
            case,
        })
    }

    /// `convertNumeric($row)`.
    pub fn convert_numeric(&self, row: Option<PhpArray>) -> Option<PhpArray> {
        row.map(|row| self.convert_row(row, None))
    }

    /// `convertAssociative($row)`.
    pub fn convert_associative(&self, row: Option<PhpArray>) -> Option<PhpArray> {
        row.map(|row| self.convert_row(row, self.case))
    }

    /// `convertOne($value)`.
    pub fn convert_one(&self, value: Option<Value>) -> Option<Value> {
        value.map(|value| self.convert_value(value))
    }

    /// `convertAllNumeric($data)`.
    pub fn convert_all_numeric(&self, data: Vec<PhpArray>) -> Vec<PhpArray> {
        data.into_iter()
            .map(|row| self.convert_row(row, None))
            .collect()
    }

    /// `convertAllAssociative($data)`.
    pub fn convert_all_associative(&self, data: Vec<PhpArray>) -> Vec<PhpArray> {
        data.into_iter()
            .map(|row| self.convert_row(row, self.case))
            .collect()
    }

    /// `convertFirstColumn($data)`.
    pub fn convert_first_column(&self, data: Vec<Value>) -> Vec<Value> {
        data.into_iter()
            .map(|value| self.convert_value(value))
            .collect()
    }

    /// `convertColumnName($name)`.
    pub fn convert_column_name(&self, name: PhpString) -> PhpString {
        match self.case {
            None => name,
            Some(case) => change_case(&name, case),
        }
    }

    /// The function that converts each value (`createConvertValue()`).
    fn convert_value(&self, mut value: Value) -> Value {
        if self.convert_empty_string_to_null
            && let Value::String(string) = &value
            && string.is_empty()
        {
            value = Value::Null;
        }

        if self.right_trim_string
            && let Value::String(string) = &value
        {
            value = Value::String(rtrim(string));
        }

        value
    }

    /// The function that converts each row (`createConvertRow()`).
    fn convert_row(&self, row: PhpArray, case: Option<i64>) -> PhpArray {
        let row: PhpArray = if self.convert_empty_string_to_null || self.right_trim_string {
            row.iter()
                .map(|(key, value)| (key.clone(), self.convert_value(value.clone())))
                .collect()
        } else {
            row
        };

        match case {
            None => row,
            Some(case) => row
                .iter()
                .map(|(key, value)| {
                    let key = match key {
                        Key::Str(name) => Key::from(change_case(name, case)),
                        Key::Int(index) => Key::Int(*index),
                    };
                    (key, value.clone())
                })
                .collect(),
        }
    }
}

/// `rtrim($value)`.
fn rtrim(string: &PhpString) -> PhpString {
    let bytes = string.as_bytes();
    let end = bytes
        .iter()
        .rposition(|byte| !matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'\0' | 0x0B))
        .map_or(0, |position| position + 1);
    PhpString::from(&bytes[..end])
}

/// `strtolower()` for `CASE_LOWER`, `strtoupper()` for `CASE_UPPER`.
fn change_case(name: &PhpString, case: i64) -> PhpString {
    let bytes = name.as_bytes();
    if case == Converter::CASE_UPPER {
        PhpString::from(bytes.to_ascii_uppercase())
    } else {
        PhpString::from(bytes.to_ascii_lowercase())
    }
}
