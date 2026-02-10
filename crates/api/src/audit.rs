//! Structured audit logging for security-relevant events.
//!
//! This module provides infrastructure for audit logging using NATS JetStream
//! for reliable, persistent event delivery. Events are published to the AUDIT_EVENTS
//! stream and consumed by a background worker for database persistence.

use crate::storage::postgres::audit as audit_storage;
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::time::SystemTime;
use tracing::{info, warn};

/// NATS subject for audit events
pub const AUDIT_EVENTS_SUBJECT: &str = "audit.events";

/// NATS stream name for audit events
pub const AUDIT_EVENTS_STREAM: &str = "AUDIT_EVENTS";

/// Audit event types for security-relevant operations
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// User authentication events
    AuthFailed,

    /// Resource access events
    ResourceCreate,
    ResourceRead,
    ResourceUpdate,
    ResourceDelete,

    /// Data access events
    ChatMessage,
    SearchRequest,
    FileDownload,
    ConfigurationChange,

    /// Marketplace events
    MarketplaceGrab,

    /// Security events
    UnauthorizedAccess,
    ValidationFailed,
    RateLimitExceeded,

    /// System events
    SystemError,
}

/// Resource types for audit logging
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceType {
    Collection,
    Dataset,
    Embedder,
    Transform,
    Visualization,
    LlmProvider,
    Session,
}

/// Outcome of the audited action
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
    Denied,
    /// Request was allowed despite degraded conditions
    Allowed,
}

/// Audit log entry for security events
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct AuditEvent {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Type of audit event
    pub event_type: AuditEventType,
    /// Outcome of the action
    pub outcome: AuditOutcome,
    /// Hashed user ID for infrastructure consistency
    pub user: String,
    /// Display name for human-readable audit logs
    pub user_display: String,
    /// Resource type being accessed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<ResourceType>,
    /// Resource identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,
    /// Additional details/reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AuditEvent {
    /// Create a new audit event with the current timestamp
    /// `user` is the hashed user ID, `user_display` is the real username for display
    pub fn new(
        event_type: AuditEventType,
        outcome: AuditOutcome,
        user: impl Into<String>,
        user_display: impl Into<String>,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| {
                chrono::DateTime::from_timestamp(d.as_secs() as i64, d.subsec_nanos())
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        Self {
            timestamp,
            event_type,
            outcome,
            user: user.into(),
            user_display: user_display.into(),
            resource_type: None,
            resource_id: None,
            details: None,
        }
    }

    /// Add resource information
    pub fn with_resource(
        mut self,
        resource_type: ResourceType,
        resource_id: impl Into<String>,
    ) -> Self {
        self.resource_type = Some(resource_type);
        self.resource_id = Some(resource_id.into());
        self
    }

    /// Add additional details
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    /// Log this audit event
    /// Uses structured logging for easy parsing and aggregation
    pub fn log(&self) {
        // Use tracing to emit the audit event as a structured log
        // The "audit" target allows filtering/routing to separate audit log
        match self.outcome {
            AuditOutcome::Success | AuditOutcome::Allowed => {
                info!(
                    target: "audit",
                    event_type = ?self.event_type,
                    outcome = ?self.outcome,
                    user = %self.user,
                    resource_type = ?self.resource_type,
                    resource_id = ?self.resource_id,
                    details = ?self.details,
                    "AUDIT"
                );
            }
            AuditOutcome::Failure | AuditOutcome::Denied => {
                warn!(
                    target: "audit",
                    event_type = ?self.event_type,
                    outcome = ?self.outcome,
                    user = %self.user,
                    resource_type = ?self.resource_type,
                    resource_id = ?self.resource_id,
                    details = ?self.details,
                    "AUDIT"
                );
            }
        }
    }

    /// Store this audit event in the database for long-term retention and querying
    pub async fn store(&self, pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
        audit_storage::store_audit_event_simple(pool, self).await
    }
}

/// Convenience functions for common audit events
pub mod events {
    use super::*;
    use actix_web::HttpRequest;
    use std::sync::OnceLock;

    // Global clients for audit event handling
    static AUDIT_DB_POOL: OnceLock<Pool<Postgres>> = OnceLock::new();
    static AUDIT_NATS_CLIENT: OnceLock<async_nats::Client> = OnceLock::new();

    /// Initialize the audit infrastructure (database pool and NATS client)
    /// Call this during application startup
    pub fn init(pool: Pool<Postgres>, nats_client: async_nats::Client) {
        let _ = AUDIT_DB_POOL.set(pool);
        let _ = AUDIT_NATS_CLIENT.set(nats_client);
    }

    /// Get the audit database pool if initialized
    pub fn get_db_pool() -> Option<&'static Pool<Postgres>> {
        AUDIT_DB_POOL.get()
    }

    /// Get the NATS client if initialized
    fn get_nats_client() -> Option<&'static async_nats::Client> {
        AUDIT_NATS_CLIENT.get()
    }

    /// Publish audit event to NATS stream for async processing
    /// Returns true if successfully published, false if NATS unavailable
    fn publish_audit_event(event: &AuditEvent) -> bool {
        if let Some(nats) = get_nats_client() {
            match serde_json::to_vec(event) {
                Ok(payload) => {
                    let nats = nats.clone();
                    tokio::spawn(async move {
                        if let Err(e) = nats.publish(AUDIT_EVENTS_SUBJECT, payload.into()).await {
                            warn!(
                                target: "audit",
                                error = %e,
                                "Failed to publish audit event to NATS"
                            );
                        }
                    });
                    true
                }
                Err(e) => {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to serialize audit event"
                    );
                    false
                }
            }
        } else {
            false
        }
    }

    /// Log a successful resource creation with request context
    pub fn resource_created_with_request(
        _req: &HttpRequest,
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ResourceCreate,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a successful resource read
    pub fn resource_read(
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ResourceRead,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a successful resource update
    pub fn resource_updated(
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ResourceUpdate,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a successful resource deletion with request context
    pub fn resource_deleted_with_request(
        _req: &HttpRequest,
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ResourceDelete,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log an authentication failure
    pub fn auth_failed(user_id: &str, user_display: &str, reason: &str) {
        let event = AuditEvent::new(
            AuditEventType::AuthFailed,
            AuditOutcome::Failure,
            user_id,
            user_display,
        )
        .with_details(reason);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log an unauthorized access attempt
    pub fn unauthorized_access(
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
        reason: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::UnauthorizedAccess,
            AuditOutcome::Denied,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id)
        .with_details(reason);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a validation failure
    pub fn validation_failed(user_id: &str, user_display: &str, field: &str, reason: &str) {
        let event = AuditEvent::new(
            AuditEventType::ValidationFailed,
            AuditOutcome::Failure,
            user_id,
            user_display,
        )
        .with_details(format!("{}: {}", field, reason));
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a chat message
    pub fn chat_message_sent(
        _req: &HttpRequest,
        user_id: &str,
        user_display: &str,
        session_id: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ChatMessage,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(ResourceType::Session, session_id);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a search request
    pub fn search_request(
        _req: &HttpRequest,
        user_id: &str,
        user_display: &str,
        collection_ids: &[String],
    ) {
        let event = AuditEvent::new(
            AuditEventType::SearchRequest,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_details(format!("collections: {}", collection_ids.join(", ")));
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a file download
    pub fn file_downloaded(user_id: &str, user_display: &str, collection_id: i32, filename: &str) {
        let event = AuditEvent::new(
            AuditEventType::FileDownload,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(ResourceType::Collection, collection_id.to_string())
        .with_details(filename);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a configuration change (e.g., embedder/LLM API key update)
    pub fn configuration_changed(
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
        field: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ConfigurationChange,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id)
        .with_details(format!("field: {}", field));
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a marketplace operation (grab collection, grab dataset, etc.)
    pub fn marketplace_grab(
        user_id: &str,
        user_display: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::MarketplaceGrab,
            AuditOutcome::Success,
            user_id,
            user_display,
        )
        .with_resource(resource_type, resource_id);
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }

    /// Log a file validation failure during upload
    pub fn file_validation_failed(
        user_id: &str,
        user_display: &str,
        collection_id: i32,
        filename: &str,
        reason: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ValidationFailed,
            AuditOutcome::Failure,
            user_id,
            user_display,
        )
        .with_resource(ResourceType::Collection, collection_id.to_string())
        .with_details(format!("file: {}; reason: {}", filename, reason));
        event.log();

        // Try to publish to NATS, fall back to direct database write if unavailable
        if !publish_audit_event(&event)
            && let Some(pool) = get_db_pool()
        {
            let event_clone = event.clone();
            tokio::spawn(async move {
                if let Err(e) = event_clone.store(pool).await {
                    warn!(
                        target: "audit",
                        error = %e,
                        "Failed to store audit event in database"
                    );
                }
            });
        }
    }
}
