//! Port of Doctrine\DBAL\Platforms\MySQL\Comparator.php: compares schemas
//! in the context of `MySQL` platform.
//!
//! PHP's subclass is a newtype over `schema::Comparator` (its methods by
//! `Deref`) whose `compareTables()` normalizes both tables first.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/MySQL/Comparator.php

use std::ops::Deref;

use php_value::{Key, PhpArray, Value};

use super::{CharsetMetadataProvider, CollationMetadataProvider, DefaultTableOptions};
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;
use crate::schema::{Comparator as BaseComparator, ComparatorConfig, Table};

/// Compares schemas in the context of `MySQL` platform.
///
/// In `MySQL`, unless specified explicitly, the column's character set and
/// collation are inherited from its containing table. So during
/// comparison, an omitted value and the value that matches the default
/// value of table in the desired schema must be considered equal.
#[derive(Debug)]
pub struct Comparator<'a>(BaseComparator<'a>);

impl<'a> Comparator<'a> {
    /// `@internal` The comparator can be only instantiated by a schema
    /// manager. `$platform` is an `AbstractMySQLPlatform`.
    pub fn new(
        platform: &'a dyn AbstractPlatform,
        charset_metadata_provider: Box<dyn CharsetMetadataProvider + 'a>,
        collation_metadata_provider: Box<dyn CollationMetadataProvider + 'a>,
        default_table_options: DefaultTableOptions,
        config: ComparatorConfig,
    ) -> Self {
        let normalizer = Normalizer {
            charset_metadata_provider,
            collation_metadata_provider,
            default_table_options,
        };
        Self(BaseComparator::with_normalizer(
            platform,
            config,
            Box::new(move |table| normalizer.normalize_table(table)),
        ))
    }
}

impl<'a> Deref for Comparator<'a> {
    type Target = BaseComparator<'a>;

    fn deref(&self) -> &BaseComparator<'a> {
        &self.0
    }
}

struct Normalizer<'a> {
    charset_metadata_provider: Box<dyn CharsetMetadataProvider + 'a>,
    collation_metadata_provider: Box<dyn CollationMetadataProvider + 'a>,
    default_table_options: DefaultTableOptions,
}

/// A `string` parameter of a provider method under `strict_types`.
fn string_argument<'v>(
    value: &'v Value,
    method: &str,
    parameter: &str,
) -> Result<&'v str, Exception> {
    match value {
        Value::String(s) => Ok(s.to_str().unwrap_or_default()),
        other => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "{method}(): Argument #1 (${parameter}) must be of type string, {} given",
                other.value_name()
            ),
        )),
    }
}

fn optional(value: Option<String>) -> Value {
    value.map_or(Value::Null, Value::from)
}

/// `(string) $value` as `array_diff_assoc()` compares it.
fn string_of(value: &Value) -> Vec<u8> {
    match value {
        Value::Array(_) => b"Array".to_vec(),
        other => other
            .to_php_string_or_stringable()
            .map(|s| s.as_bytes().to_vec())
            .unwrap_or_default(),
    }
}

/// `$a === $b` of two arrays: the same pairs in the same order.
fn identical(a: &PhpArray, b: &PhpArray) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b.iter())
            .all(|((ka, va), (kb, vb))| ka == kb && va.identical(vb))
}

const CHARSET_METHOD: &str =
    "Doctrine\\DBAL\\Platforms\\MySQL\\CharsetMetadataProvider::getDefaultCharsetCollation";
const COLLATION_METHOD: &str =
    "Doctrine\\DBAL\\Platforms\\MySQL\\CollationMetadataProvider::getCollationCharset";

impl Normalizer<'_> {
    fn default_charset_collation(&self, charset: &Value) -> Result<Value, Exception> {
        let charset = string_argument(charset, CHARSET_METHOD, "charset")?;
        Ok(optional(
            self.charset_metadata_provider
                .get_default_charset_collation(charset)?,
        ))
    }

    fn collation_charset(&self, collation: &Value) -> Result<Value, Exception> {
        let collation = string_argument(collation, COLLATION_METHOD, "collation")?;
        Ok(optional(
            self.collation_metadata_provider
                .get_collation_charset(collation)?,
        ))
    }

    /// `normalizeTable($table)`.
    fn normalize_table(&self, table: &Table) -> Result<Table, Exception> {
        let mut charset = table.get_option("charset");
        let mut collation = table.get_option("collation");

        match (charset.is_null(), collation.is_null()) {
            (true, false) => charset = self.collation_charset(&collation)?,
            (false, true) => collation = self.default_charset_collation(&charset)?,
            (true, true) => {
                charset = Value::from(self.default_table_options.get_charset());
                collation = Value::from(self.default_table_options.get_collation());
            }
            (false, false) => {}
        }

        let mut table_options = PhpArray::new();
        table_options.insert("charset", charset);
        table_options.insert("collation", collation);

        let mut table = table.clone();

        for column in table.columns_mut() {
            let original_options = column.get_platform_options().clone();
            let normalized_options = self.normalize_options(&original_options)?;

            let mut override_options = PhpArray::new();
            for (key, value) in normalized_options.iter() {
                let same = table_options
                    .get(key)
                    .is_some_and(|other| string_of(other) == string_of(value));
                if !same {
                    override_options.insert(key.clone(), value.clone());
                }
            }

            if !identical(&override_options, &original_options) {
                column.set_platform_options_inner(override_options);
            }
        }

        Ok(table)
    }

    /// `normalizeOptions($options)`.
    fn normalize_options(&self, options: &PhpArray) -> Result<PhpArray, Exception> {
        let isset = |name: &str| !matches!(options.get(&Key::from(name)), None | Some(Value::Null));
        let mut options = options.clone();

        if isset("charset") && !isset("collation") {
            let charset = options
                .get(&Key::from("charset"))
                .cloned()
                .unwrap_or_default();
            options.insert("collation", self.default_charset_collation(&charset)?);
        } else if isset("collation") && !isset("charset") {
            let collation = options
                .get(&Key::from("collation"))
                .cloned()
                .unwrap_or_default();
            options.insert("charset", self.collation_charset(&collation)?);
        }

        Ok(options)
    }
}
