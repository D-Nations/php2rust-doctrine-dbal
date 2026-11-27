//! Port of Doctrine\DBAL\Driver\OCI8\ExecutionMode.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/OCI8/ExecutionMode.php

/// `final class ExecutionMode`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionMode {
    is_auto_commit_enabled: bool,
}

impl Default for ExecutionMode {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionMode {
    #[must_use]
    pub fn new() -> Self {
        Self {
            is_auto_commit_enabled: true,
        }
    }

    /// `enableAutoCommit()`.
    pub fn enable_auto_commit(&mut self) {
        self.is_auto_commit_enabled = true;
    }

    /// `disableAutoCommit()`.
    pub fn disable_auto_commit(&mut self) {
        self.is_auto_commit_enabled = false;
    }

    /// `isAutoCommitEnabled()`.
    #[must_use]
    pub fn is_auto_commit_enabled(&self) -> bool {
        self.is_auto_commit_enabled
    }
}
