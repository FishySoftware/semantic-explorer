//! Structured audit logging for security-relevant events.
//!
//! This module provides infrastructure for audit logging as part of Phase 5.4
//! migration tasks. The utilities are ready for integration into handlers.

use serde::Serialize;
use std::time::SystemTime;
use tracing::{info, warn};

/// Audit event types for security-relevant operations
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Some variants reserved for future extensibility
pub enum AuditEventType {
    /// User authentication events
    AuthLogin,
    AuthLogout,
    AuthFailed,

    /// Resource access events
    ResourceCreate,
    ResourceRead,
    ResourceUpdate,
    ResourceDelete,

    /// Administrative actions
    AdminAction,
    ConfigChange,

    /// Security events
    UnauthorizedAccess,
    ValidationFailed,
}

/// Resource types for audit logging
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Some variants reserved for future extensibility
pub enum ResourceType {
    Collection,
    Dataset,
    Embedder,
    Transform,
    Visualization,
    LlmProvider,
    User,
    Session,
}

/// Outcome of the audited action
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Success,
    Failure,
    Denied,
}

/// Audit log entry for security events
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Type of audit event
    pub event_type: AuditEventType,
    /// Outcome of the action
    pub outcome: AuditOutcome,
    /// Username or "anonymous" if not authenticated
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
    #[allow(dead_code)] // Available for handlers that need explicit request ID correlation
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
}

/// Convenience functions for common audit events
pub mod events {
    use super::*;
    use crate::middleware::RequestId;
    use actix_web::{HttpMessage, HttpRequest};

    /// Extract request ID from an HttpRequest if available
    #[allow(dead_code)] // Available for handlers that need explicit request ID
    fn get_request_id(req: &HttpRequest) -> Option<String> {
        req.extensions().get::<RequestId>().map(|r| r.0.clone())
    }

    /// Log a successful resource creation
    pub fn resource_created(user: &str, resource_type: ResourceType, resource_id: &str) {
        AuditEvent::new(AuditEventType::ResourceCreate, AuditOutcome::Success, user)
            .with_resource(resource_type, resource_id)
            .log();
    }

    /// Log a successful resource creation with request context
    #[allow(dead_code)] // Available for handlers that have access to HttpRequest
    pub fn resource_created_with_request(
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
        req: &HttpRequest,
    ) {
        let mut event =
            AuditEvent::new(AuditEventType::ResourceCreate, AuditOutcome::Success, user)
                .with_resource(resource_type, resource_id);
        if let Some(id) = get_request_id(req) {
            event = event.with_request_id(id);
        }
        event.log();
    }

    /// Log a successful resource read
    pub fn resource_read(user: &str, resource_type: ResourceType, resource_id: &str) {
        AuditEvent::new(AuditEventType::ResourceRead, AuditOutcome::Success, user)
            .with_resource(resource_type, resource_id)
            .log();
    }

    /// Log a successful resource update
    pub fn resource_updated(user: &str, resource_type: ResourceType, resource_id: &str) {
        AuditEvent::new(AuditEventType::ResourceUpdate, AuditOutcome::Success, user)
            .with_resource(resource_type, resource_id)
            .log();
    }

    /// Log a successful resource deletion
    pub fn resource_deleted(user: &str, resource_type: ResourceType, resource_id: &str) {
        AuditEvent::new(AuditEventType::ResourceDelete, AuditOutcome::Success, user)
            .with_resource(resource_type, resource_id)
            .log();
    }

    /// Log an authentication failure
    pub fn auth_failed(user: &str, reason: &str, client_ip: Option<&str>) {
        let mut event = AuditEvent::new(AuditEventType::AuthFailed, AuditOutcome::Failure, user)
            .with_details(reason);
        if let Some(ip) = client_ip {
            event = event.with_client_ip(ip);
        }
        event.log();
    }

    /// Log an unauthorized access attempt
    pub fn unauthorized_access(
        user: &str,
        resource_type: ResourceType,
        resource_id: &str,
        reason: &str,
    ) {
        AuditEvent::new(
            AuditEventType::UnauthorizedAccess,
            AuditOutcome::Denied,
            user,
        )
        .with_resource(resource_type, resource_id)
        .with_details(reason)
        .log();
    }

    /// Log a validation failure
    pub fn validation_failed(user: &str, field: &str, reason: &str) {
        AuditEvent::new(
            AuditEventType::ValidationFailed,
            AuditOutcome::Failure,
            user,
        )
        .with_details(format!("{}: {}", field, reason))
        .log();
    }
}
