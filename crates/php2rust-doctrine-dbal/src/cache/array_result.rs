//! Port of Doctrine\DBAL\Cache\ArrayResult.php (`@internal` to the caching
//! layer): a driver result over rows held in memory.
//!
//! `__serialize()` and `__unserialize()` are [`ArrayResult::serialize`]
//! and [`ArrayResult::unserialize`], PHP's `serialize($result)` and
//! `unserialize($data)` of one: the 4.2 format (column names and rows)
//! and the 4.1 format (rows by column name) are read; the row pointer is
//! not kept.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Cache/ArrayResult.php

use php_value::constants::Registry;
use php_value::serialize::{Classes, serialize, unserialize};
use php_value::{Key, PhpArray, PhpString, Value};

use crate::Exception;
use crate::driver::{FetchUtils, Result as DriverResult};

/// `Doctrine\DBAL\Cache\ArrayResult`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayResult {
    column_names: Vec<PhpString>,
    rows: Vec<PhpArray>,
    num: usize,
}

impl ArrayResult {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Cache\\ArrayResult";

    /// `$columnNames`: the names of the result columns (must be
    /// non-empty); `$rows`: lists with as many values as there are
    /// columns.
    pub fn new(column_names: Vec<PhpString>, rows: Vec<PhpArray>) -> Self {
        Self {
            column_names,
            rows,
            num: 0,
        }
    }

    /// `serialize($this)`, with `__serialize()`:
    /// `[$this->columnNames, $this->rows]`.
    pub fn serialize(&self) -> PhpString {
        let names = PhpArray::list(self.column_names.iter().cloned().map(Value::String));
        let rows = PhpArray::list(self.rows.iter().cloned().map(Value::Array));
        let data = PhpArray::list([Value::Array(names), Value::Array(rows)]);
        let array = serialize(&Value::Array(data));
        // `a:2:{...}` as the object's data: `O:31:"...ArrayResult":2:{...}`.
        let mut out = format!("O:{}:\"{}\":", Self::CLASS.len(), Self::CLASS).into_bytes();
        out.extend_from_slice(array.as_bytes().get(2..).unwrap_or_default());
        PhpString::from(out)
    }

    /// `unserialize($data)` of an `ArrayResult`, with `__unserialize()`;
    /// `None` when the data is not one.
    pub fn unserialize(data: &[u8]) -> Option<Self> {
        let mut classes = Classes::new();
        classes.allow(Self::CLASS);
        let Value::Object(object) = unserialize(data, &classes, &Registry::default())? else {
            return None;
        };
        if object.class() != Self::CLASS {
            return None;
        }
        Self::from_serialized(&object.properties())
    }

    /// `__unserialize($data)`.
    fn from_serialized(data: &PhpArray) -> Option<Self> {
        let at = |index: i64| {
            data.get(&Key::Int(index))
                .or_else(|| data.get(&Key::from(index.to_string().as_str())))
        };
        // Handle objects serialized with DBAL 4.1 and earlier.
        let legacy_key = format!("\0{}\0data", Self::CLASS);
        if let Some(Value::Array(legacy)) = data.get(&Key::from(legacy_key.as_str())) {
            let first = legacy.iter().next().and_then(|(_, row)| match row {
                Value::Array(row) => Some(row),
                _ => None,
            });
            let column_names = first.map_or_else(Vec::new, |row| {
                row.iter()
                    .map(|(key, _)| match key {
                        Key::Int(i) => PhpString::from(i.to_string()),
                        Key::Str(s) => s.clone(),
                    })
                    .collect()
            });
            let rows = legacy
                .iter()
                .map(|(_, row)| match row {
                    Value::Array(row) => PhpArray::list(row.iter().map(|(_, v)| v.clone())),
                    _ => PhpArray::new(),
                })
                .collect();
            return Some(Self::new(column_names, rows));
        }
        let (Some(Value::Array(names)), Some(Value::Array(rows))) = (at(0), at(1)) else {
            return None;
        };
        let column_names = names
            .iter()
            .map(|(_, name)| match name {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let rows = rows
            .iter()
            .map(|(_, row)| match row {
                Value::Array(row) => Some(row.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self::new(column_names, rows))
    }

    /// The next row (`fetch()`), or `None`.
    fn fetch(&mut self) -> Option<PhpArray> {
        let row = self.rows.get(self.num)?.clone();
        self.num += 1;
        Some(row)
    }
}

impl DriverResult for ArrayResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self.fetch())
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        let Some(row) = self.fetch() else {
            return Ok(None);
        };
        // `array_combine($this->columnNames, $row)`.
        let mut combined = PhpArray::new();
        for (name, (_, value)) in self.column_names.iter().zip(row.iter()) {
            combined.insert(Key::from(name.clone()), value.clone());
        }
        Ok(Some(combined))
    }

    fn fetch_one(&mut self) -> Result<Option<Value>, Exception> {
        Ok(self
            .fetch()
            .map(|row| row.get(&Key::Int(0)).cloned().unwrap_or(Value::Null)))
    }

    fn fetch_all_numeric(&mut self) -> Result<Vec<PhpArray>, Exception> {
        FetchUtils::fetch_all_numeric(self)
    }

    fn fetch_all_associative(&mut self) -> Result<Vec<PhpArray>, Exception> {
        FetchUtils::fetch_all_associative(self)
    }

    fn fetch_first_column(&mut self) -> Result<Vec<Value>, Exception> {
        FetchUtils::fetch_first_column(self)
    }

    fn row_count(&self) -> Result<i64, Exception> {
        Ok(i64::try_from(self.rows.len()).unwrap_or(i64::MAX))
    }

    fn column_count(&self) -> Result<usize, Exception> {
        Ok(self.column_names.len())
    }

    fn free(&mut self) {
        self.rows.clear();
    }

    fn get_column_name(&self, index: i64) -> Option<Result<PhpString, Exception>> {
        let name = usize::try_from(index)
            .ok()
            .and_then(|index| self.column_names.get(index));
        Some(
            name.cloned()
                .ok_or_else(|| Exception::invalid_column_index(index, None)),
        )
    }

    fn class(&self) -> &str {
        Self::CLASS
    }
}
