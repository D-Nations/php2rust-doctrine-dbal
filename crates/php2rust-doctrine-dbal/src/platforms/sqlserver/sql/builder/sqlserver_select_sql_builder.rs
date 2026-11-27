//! Port of Doctrine\DBAL\Platforms\SQLServer\SQL\Builder\SQLServerSelectSQLBuilder.php:
//! the lock hints after FROM (`WITH (UPDLOCK, ROWLOCK[, READPAST])`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/SQLServer/SQL/Builder/SQLServerSelectSQLBuilder.php

use crate::exception::Exception;
use crate::platforms::AbstractPlatform;
use crate::platforms::SQLServerPlatform;
use crate::query::{ConflictResolutionMode, SelectQuery};
use crate::sql::builder::SelectSQLBuilder;

/// The SELECT builder of `SQLServerPlatform`.
pub struct SQLServerSelectSQLBuilder<'p> {
    platform: &'p SQLServerPlatform,
}

impl<'p> SQLServerSelectSQLBuilder<'p> {
    /// `@internal` The SQL builder should be instantiated only by database
    /// platforms.
    pub fn new(platform: &'p SQLServerPlatform) -> Self {
        Self { platform }
    }
}

impl SelectSQLBuilder for SQLServerSelectSQLBuilder<'_> {
    fn build_sql(&self, query: &SelectQuery) -> Result<String, Exception> {
        let mut parts = vec!["SELECT".to_owned()];

        if query.is_distinct() {
            parts.push("DISTINCT".to_owned());
        }

        parts.push(query.get_columns().join(", "));

        let from = query.get_from();

        if !from.is_empty() {
            parts.push(format!("FROM {}", from.join(", ")));
        }

        if let Some(for_update) = query.get_for_update() {
            let mut with = vec!["UPDLOCK", "ROWLOCK"];

            if for_update.get_conflict_resolution_mode() == ConflictResolutionMode::SkipLocked {
                with.push("READPAST");
            }

            parts.push(format!("WITH ({})", with.join(", ")));
        }

        if let Some(r#where) = query.get_where() {
            parts.push(format!("WHERE {where}"));
        }

        let group_by = query.get_group_by();

        if !group_by.is_empty() {
            parts.push(format!("GROUP BY {}", group_by.join(", ")));
        }

        if let Some(having) = query.get_having() {
            parts.push(format!("HAVING {having}"));
        }

        let order_by = query.get_order_by();

        if !order_by.is_empty() {
            parts.push(format!("ORDER BY {}", order_by.join(", ")));
        }

        let mut sql = parts.join(" ");
        let limit = query.get_limit();

        if limit.is_defined() {
            sql = self.platform.modify_limit_query(
                &sql,
                limit.get_max_results(),
                limit.get_first_result(),
            )?;
        }

        Ok(sql)
    }
}
