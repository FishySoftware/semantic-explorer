//! Structured audit logging for security-relevant events.
//!
//! This module provides infrastructure for audit logging using NATS JetStream
//! for reliable, persistent event delivery. Events are published to the AUDIT_EVENTS
//! stream and consumed by a background worker for database persistence.

use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::time::SystemTime;
use tracing::{info, warn};

// SQL Queries
const INSERT_AUDIT_EVENT_QUERY: &str = r#"
    INSERT INTO audit_events (
        timestamp,
        event_type,
        outcome,
        username,
        request_id,
        client_ip,
        resource_type,
        resource_id,
        details
    )
    VALUES (
        $1::timestamp with time zone, $2, $3, $4, $5, $6, $7, $8, $9
    )
"#;

// NATS configuration for audit events
pub const AUDIT_EVENTS_SUBJECT: &str = "audit.events";
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
    /// Username
    pub user: String,
    /// Request ID for correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    /// Client IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_ip: Option<String>,
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
    pub fn new(event_type: AuditEventType, outcome: AuditOutcome, user: impl Into<String>) -> Self {
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
            request_id: None,
            client_ip: None,
            resource_type: None,
            resource_id: None,
            details: None,
        }
    }

    /// Add request ID for correlation
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Add client IP address
    pub fn with_client_ip(mut self, ip: impl Into<String>) -> Self {
        self.client_ip = Some(ip.into());
        self
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
            AuditOutcome::Success => {
                info!(
                    target: "audit",
                    event_type = ?self.event_type,
                    outcome = "success",
                    user = %self.user,
                    request_id = ?self.request_id,
                    client_ip = ?self.client_ip,
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
                    request_id = ?self.request_id,
                    client_ip = ?self.client_ip,
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
        sqlx::query(INSERT_AUDIT_EVENT_QUERY)
            .bind(&self.timestamp)
            .bind(format!("{:?}", self.event_type))
            .bind(format!("{:?}", self.outcome))
            .bind(&self.user)
            .bind(&self.request_id)
            .bind(&self.client_ip)
            .bind(self.resource_type.as_ref().map(|rt| format!("{:?}", rt)))
            .bind(&self.resource_id)
            .bind(&self.details)
            .execute(pool)
            .await?;

        Ok(())
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

    /// Extract request ID from an HttpRequest if available
    fn get_request_id(_req: &HttpRequest) -> Option<String> {
        // RequestId would be available via extensions if RequestIdMiddleware is active
        // For now, generate a unique ID per request
        None
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
        req: &HttpRequest,
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let mut event =
            AuditEvent::new(AuditEventType::ResourceCreate, AuditOutcome::Success, user)
                .with_resource(resource_type, resource_id);
        if let Some(id) = get_request_id(req) {
            event = event.with_request_id(id);
        }
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
    pub fn resource_read(user: &str, resource_type: ResourceType, resource_id: &str) {
        let event = AuditEvent::new(AuditEventType::ResourceRead, AuditOutcome::Success, user)
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
    pub fn resource_updated(user: &str, resource_type: ResourceType, resource_id: &str) {
        let event = AuditEvent::new(AuditEventType::ResourceUpdate, AuditOutcome::Success, user)
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
        req: &HttpRequest,
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let mut event =
            AuditEvent::new(AuditEventType::ResourceDelete, AuditOutcome::Success, user)
                .with_resource(resource_type, resource_id);
        if let Some(id) = get_request_id(req) {
            event = event.with_request_id(id);
        }
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
    pub fn auth_failed(user: &str, reason: &str, client_ip: Option<&str>) {
        let mut event = AuditEvent::new(AuditEventType::AuthFailed, AuditOutcome::Failure, user)
            .with_details(reason);
        if let Some(ip) = client_ip {
            event = event.with_client_ip(ip);
        }
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
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
        reason: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::UnauthorizedAccess,
            AuditOutcome::Denied,
            user,
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
    pub fn validation_failed(user: &str, field: &str, reason: &str) {
        let event = AuditEvent::new(
            AuditEventType::ValidationFailed,
            AuditOutcome::Failure,
            user,
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
    pub fn chat_message_sent(req: &HttpRequest, user: &str, session_id: &str) {
        let mut event = AuditEvent::new(AuditEventType::ChatMessage, AuditOutcome::Success, user)
            .with_resource(ResourceType::Session, session_id);
        if let Some(request_id) = get_request_id(req) {
            event = event.with_request_id(request_id);
        }
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
    pub fn search_request(req: &HttpRequest, user: &str, collection_ids: &[String]) {
        let mut event = AuditEvent::new(AuditEventType::SearchRequest, AuditOutcome::Success, user)
            .with_details(format!("collections: {}", collection_ids.join(", ")));
        if let Some(request_id) = get_request_id(req) {
            event = event.with_request_id(request_id);
        }
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
    pub fn file_downloaded(req: &HttpRequest, user: &str, collection_id: i32, filename: &str) {
        let mut event = AuditEvent::new(AuditEventType::FileDownload, AuditOutcome::Success, user)
            .with_resource(ResourceType::Collection, collection_id.to_string())
            .with_details(filename);
        if let Some(request_id) = get_request_id(req) {
            event = event.with_request_id(request_id);
        }
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
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
        field: &str,
    ) {
        let event = AuditEvent::new(
            AuditEventType::ConfigurationChange,
            AuditOutcome::Success,
            user,
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
        req: &HttpRequest,
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
    ) {
        let mut event =
            AuditEvent::new(AuditEventType::MarketplaceGrab, AuditOutcome::Success, user)
                .with_resource(resource_type, resource_id);
        if let Some(request_id) = get_request_id(req) {
            event = event.with_request_id(request_id);
        }
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
    pub fn file_validation_failed(user: &str, collection_id: i32, filename: &str, reason: &str) {
        let event = AuditEvent::new(
            AuditEventType::ValidationFailed,
            AuditOutcome::Failure,
            user,
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
