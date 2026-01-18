//! Owner information wrapper for database operations.
//!
//! This module provides a simple struct to group owner_id and owner_display_name
//! fields that are commonly passed together throughout the codebase.

use serde::{Deserialize, Serialize};

/// Owner information containing both the hashed owner ID and display name.
///
/// This struct groups together the owner_id (hashed, infrastructure-safe) and
/// owner_display_name (original username) that are commonly passed together
/// to database functions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerInfo {
    /// Hashed owner ID used for database records, NATS subjects, and S3 paths
    pub owner_id: String,
    /// Original username for display purposes
    pub owner_display_name: String,
}

impl OwnerInfo {
    /// Create a new OwnerInfo instance
    pub fn new(owner_id: impl Into<String>, owner_display_name: impl Into<String>) -> Self {
        Self {
            owner_id: owner_id.into(),
            owner_display_name: owner_display_name.into(),
        }
    }
}
