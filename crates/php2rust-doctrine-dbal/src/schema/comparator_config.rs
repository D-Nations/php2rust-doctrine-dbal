//! Port of Doctrine\DBAL\Schema\ComparatorConfig.php: the options of a
//! `Comparator`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/ComparatorConfig.php

/// `ComparatorConfig`. `Default` gives `new ComparatorConfig()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComparatorConfig {
    detect_renamed_columns: bool,
    detect_renamed_indexes: bool,
    report_modified_indexes: bool,
}

impl Default for ComparatorConfig {
    fn default() -> Self {
        Self::new(true, true, true)
    }
}

impl ComparatorConfig {
    /// `new ComparatorConfig($detectRenamedColumns, $detectRenamedIndexes,
    /// $reportModifiedIndexes)`.
    pub fn new(
        detect_renamed_columns: bool,
        detect_renamed_indexes: bool,
        report_modified_indexes: bool,
    ) -> Self {
        Self {
            detect_renamed_columns,
            detect_renamed_indexes,
            report_modified_indexes,
        }
    }

    #[must_use]
    pub fn with_detect_renamed_columns(self, detect_renamed_columns: bool) -> Self {
        Self::new(
            detect_renamed_columns,
            self.detect_renamed_indexes,
            self.report_modified_indexes,
        )
    }

    pub fn get_detect_renamed_columns(&self) -> bool {
        self.detect_renamed_columns
    }

    #[must_use]
    pub fn with_detect_renamed_indexes(self, detect_renamed_indexes: bool) -> Self {
        Self::new(
            self.detect_renamed_columns,
            detect_renamed_indexes,
            self.report_modified_indexes,
        )
    }

    pub fn get_detect_renamed_indexes(&self) -> bool {
        self.detect_renamed_indexes
    }

    #[must_use]
    pub fn with_report_modified_indexes(self, report_modified_indexes: bool) -> Self {
        Self::new(
            self.detect_renamed_columns,
            self.detect_renamed_indexes,
            report_modified_indexes,
        )
    }

    /// `@internal` This method is intended solely to provide an upgrade
    /// path to DBAL 5.0.
    pub fn get_report_modified_indexes(&self) -> bool {
        self.report_modified_indexes
    }
}
