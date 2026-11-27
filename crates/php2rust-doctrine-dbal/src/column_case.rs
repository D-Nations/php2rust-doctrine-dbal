//! Port of Doctrine\DBAL\ColumnCase.php: the case to which the portability
//! layer converts column names.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/ColumnCase.php

/// `enum ColumnCase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColumnCase {
    /// Convert column names to upper case.
    Upper,
    /// Convert column names to lower case.
    Lower,
}
