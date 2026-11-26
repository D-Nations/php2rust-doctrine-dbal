//! Port of Doctrine\DBAL\Schema\ViewEditor.php.
//!
//! A by-value builder. An exception PHP throws from a setter is kept and
//! returned by `create()` (the first one wins).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/ViewEditor.php

use crate::exception::Exception;

use super::exception;
use super::name::OptionallyQualifiedName;
use super::view::View;

#[derive(Debug, Clone, Default)]
pub struct ViewEditor {
    error: Option<Exception>,
    name: Option<OptionallyQualifiedName>,
    sql: Option<String>,
}

impl ViewEditor {
    /// `@internal` Use `View::editor()` or `View::edit()`.
    pub fn new() -> Self {
        Self::default()
    }

    fn fail(&mut self, e: Exception) {
        self.error.get_or_insert(e);
    }

    #[must_use]
    pub fn set_name(mut self, name: OptionallyQualifiedName) -> Self {
        self.name = Some(name);
        self
    }

    #[must_use]
    pub fn set_unquoted_name(mut self, unqualified_name: &str, qualifier: Option<&str>) -> Self {
        match OptionallyQualifiedName::unquoted(unqualified_name, qualifier) {
            Ok(name) => self.name = Some(name),
            Err(e) => self.fail(e),
        }
        self
    }

    #[must_use]
    pub fn set_quoted_name(mut self, unqualified_name: &str, qualifier: Option<&str>) -> Self {
        match OptionallyQualifiedName::quoted(unqualified_name, qualifier) {
            Ok(name) => self.name = Some(name),
            Err(e) => self.fail(e),
        }
        self
    }

    /// `setSQL($sql)`.
    #[must_use]
    pub fn set_sql(mut self, sql: &str) -> Self {
        self.sql = Some(sql.to_owned());
        self
    }

    pub fn create(self) -> Result<View, Exception> {
        if let Some(e) = self.error {
            return Err(e);
        }

        let Some(name) = self.name else {
            return Err(exception::view_name_not_set());
        };

        let Some(sql) = self.sql else {
            return Err(exception::view_sql_not_set(&name));
        };

        Ok(View::new(&name.to_string(), &sql))
    }
}
