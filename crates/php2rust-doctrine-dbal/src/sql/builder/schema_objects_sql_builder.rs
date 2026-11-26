//! Port of Doctrine\DBAL\SQL\Builder\CreateSchemaObjectsSQLBuilder.php and
//! DropSchemaObjectsSQLBuilder.php: the SQL that creates or drops the
//! objects of a schema.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/SQL/Builder/CreateSchemaObjectsSQLBuilder.php
//! Upstream: doctrine/dbal src/SQL/Builder/DropSchemaObjectsSQLBuilder.php

use crate::exception::Exception;
use crate::platforms::AbstractPlatform;
use crate::schema::{Schema, Sequence, Table};

/// Builds the SQL that creates the objects of a schema.
#[derive(Clone, Copy)]
pub struct CreateSchemaObjectsSQLBuilder<'a> {
    platform: &'a dyn AbstractPlatform,
}

impl std::fmt::Debug for CreateSchemaObjectsSQLBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateSchemaObjectsSQLBuilder")
            .finish_non_exhaustive()
    }
}

impl<'a> CreateSchemaObjectsSQLBuilder<'a> {
    pub fn new(platform: &'a dyn AbstractPlatform) -> Self {
        Self { platform }
    }

    pub fn build_sql(&self, schema: &Schema) -> Result<Vec<String>, Exception> {
        let mut sql = self.build_namespace_statements(&schema.get_namespaces())?;
        sql.extend(self.build_sequence_statements(&schema.get_sequences())?);
        sql.extend(self.build_table_statements(&schema.get_tables())?);
        Ok(sql)
    }

    fn build_namespace_statements(&self, namespaces: &[String]) -> Result<Vec<String>, Exception> {
        let mut statements = Vec::new();
        if self.platform.supports_schemas() {
            for namespace in namespaces {
                statements.push(self.platform.get_create_schema_sql(namespace)?);
            }
        }
        Ok(statements)
    }

    fn build_table_statements(&self, tables: &[&Table]) -> Result<Vec<String>, Exception> {
        let tables: Vec<Table> = tables.iter().map(|&t| t.clone()).collect();
        self.platform.get_create_tables_sql(&tables)
    }

    fn build_sequence_statements(&self, sequences: &[&Sequence]) -> Result<Vec<String>, Exception> {
        sequences
            .iter()
            .map(|sequence| self.platform.get_create_sequence_sql(sequence))
            .collect()
    }
}

/// Builds the SQL that drops the objects of a schema.
#[derive(Clone, Copy)]
pub struct DropSchemaObjectsSQLBuilder<'a> {
    platform: &'a dyn AbstractPlatform,
}

impl std::fmt::Debug for DropSchemaObjectsSQLBuilder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DropSchemaObjectsSQLBuilder")
            .finish_non_exhaustive()
    }
}

impl<'a> DropSchemaObjectsSQLBuilder<'a> {
    pub fn new(platform: &'a dyn AbstractPlatform) -> Self {
        Self { platform }
    }

    pub fn build_sql(&self, schema: &Schema) -> Result<Vec<String>, Exception> {
        let mut sql = self.build_sequence_statements(&schema.get_sequences())?;
        sql.extend(self.build_table_statements(&schema.get_tables())?);
        Ok(sql)
    }

    fn build_table_statements(&self, tables: &[&Table]) -> Result<Vec<String>, Exception> {
        let tables: Vec<Table> = tables.iter().map(|&t| t.clone()).collect();
        self.platform.get_drop_tables_sql(&tables)
    }

    fn build_sequence_statements(&self, sequences: &[&Sequence]) -> Result<Vec<String>, Exception> {
        sequences
            .iter()
            .map(|sequence| {
                self.platform
                    .get_drop_sequence_sql(&sequence.get_quoted_name(self.platform))
            })
            .collect()
    }
}
