//! Port of Doctrine\DBAL\Schema\View.php: the representation of a
//! database view.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/View.php

use std::ops::Deref;

use crate::exception::Exception;

use super::abstract_asset::{AbstractAsset, AssetKind, NameParser};
use super::exception::invalid_state;
use super::name::{Name, OptionallyQualifiedName};
use super::view_editor::ViewEditor;

/// `Doctrine\DBAL\Schema\View`.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    asset: AbstractAsset,
    sql: String,
}

impl View {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\View";

    /// `new View($name, $sql)`.
    pub fn new(name: &str, sql: &str) -> Self {
        Self {
            asset: AbstractAsset::new(
                Self::CLASS,
                AssetKind::Named,
                NameParser::OptionallyQualified,
                Some(name),
            ),
            sql: sql.to_owned(),
        }
    }

    /// `getObjectName()`.
    pub fn get_object_name(&self) -> Result<OptionallyQualifiedName, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::OptionallyQualified(name)) => Ok(name.clone()),
            _ => Err(invalid_state::object_name_not_initialized()),
        }
    }

    pub fn get_sql(&self) -> &str {
        &self.sql
    }

    /// `View::editor()`.
    pub fn editor() -> ViewEditor {
        ViewEditor::new()
    }

    /// `edit()`: an editor initialized with the view's properties.
    pub fn edit(&self) -> Result<ViewEditor, Exception> {
        Ok(Self::editor()
            .set_name(self.get_object_name()?)
            .set_sql(&self.sql))
    }
}

impl Deref for View {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}
