//! Port of Doctrine\DBAL\Driver\OCI8\ConvertPositionalToNamedPlaceholders.php:
//! converts positional (`?`) into named placeholders (`:param<num>`).
//! Oracle does not support positional parameters, hence this visitor
//! converts all positional parameters into artificially named parameters.
//!
//! `@internal`. The SQL is bytes, as the parser gives it.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/OCI8/ConvertPositionalToNamedPlaceholders.php

use std::collections::BTreeMap;
use std::convert::Infallible;

use crate::sql::Visitor;

/// `final class ConvertPositionalToNamedPlaceholders implements Visitor`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConvertPositionalToNamedPlaceholders {
    buffer: Vec<Vec<u8>>,
    parameter_map: BTreeMap<i64, String>,
}

impl ConvertPositionalToNamedPlaceholders {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `getSQL()`.
    #[must_use]
    pub fn get_sql(&self) -> Vec<u8> {
        self.buffer.concat()
    }

    /// `getParameterMap()`: position => name.
    #[must_use]
    pub fn get_parameter_map(&self) -> &BTreeMap<i64, String> {
        &self.parameter_map
    }
}

impl Visitor for ConvertPositionalToNamedPlaceholders {
    type Error = Infallible;

    fn accept_positional_parameter(&mut self, _sql: &[u8]) -> Result<(), Self::Error> {
        let position = i64::try_from(self.parameter_map.len()).unwrap_or(i64::MAX) + 1;
        let param = format!(":param{position}");

        self.buffer.push(param.clone().into_bytes());
        self.parameter_map.insert(position, param);

        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.buffer.push(sql.to_vec());
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.buffer.push(sql.to_vec());
        Ok(())
    }
}
