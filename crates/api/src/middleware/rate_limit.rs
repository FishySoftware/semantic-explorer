//! Rate limiting middleware using Redis cluster backend.
//!
//! This middleware provides distributed rate limiting across API instances using Redis cluster.
//! It applies only to `/api/**` endpoints and excludes health check endpoints.
//! Rate limit headers are added to all API responses for transparency.

use actix_web::{
    Error, HttpMessage, HttpResponse,
    body::{BoxBody, EitherBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use prometheus::{Gauge, IntCounterVec, Registry};
use redis::cluster::ClusterClient;
use semantic_explorer_core::config::RateLimitConfig;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{error, warn};

use crate::audit::{AuditEvent, AuditEventType, AuditOutcome};

/// Rate limit metrics
#[derive(Clone)]
pub struct RateLimitMetrics {
    pub requests_total: IntCounterVec,
    pub rejections_total: IntCounterVec,
    pub redis_health: Gauge,
}

impl RateLimitMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let requests_total = IntCounterVec::new(
            prometheus::opts!(
                "rate_limit_requests_total",
                "Total number of rate limit checks by endpoint and user"
            ),
            &["endpoint_type", "user"],
        )?;
        registry.register(Box::new(requests_total.clone()))?;

        let rejections_total = IntCounterVec::new(
            prometheus::opts!(
                "rate_limit_rejections_total",
                "Total number of rate limit rejections by endpoint and user"
            ),
            &["endpoint_type", "user"],
        )?;
        registry.register(Box::new(rejections_total.clone()))?;

        let redis_health = Gauge::new(
            "rate_limit_redis_health",
            "Redis cluster health status (1=healthy, 0=unhealthy)",
        )?;
        registry.register(Box::new(redis_health.clone()))?;

        Ok(Self {
            requests_total,
            rejections_total,
            redis_health,
        })
    }
}

/// Redis-backed rate limiter using token bucket algorithm
pub struct RateLimiter {
    redis_client: ClusterClient,
    config: RateLimitConfig,
    metrics: RateLimitMetrics,
}

impl RateLimiter {
    pub fn new(
        redis_client: ClusterClient,
        config: RateLimitConfig,
        metrics: RateLimitMetrics,
    ) -> Self {
        Self {
            redis_client,
            config,
            metrics,
        }
    }

    /// Check if a request is allowed under the rate limit
    pub async fn check_rate_limit(
        &self,
        username: &str,
        endpoint_type: &str,
    ) -> Result<RateLimitResult, redis::RedisError> {
        let limit = self.get_limit_for_endpoint(endpoint_type);
        let key = format!("rate_limit:{}:{}", endpoint_type, username);

        // Get current minute as window
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window = now / 60; // 1-minute windows

        let mut conn = self.redis_client.get_async_connection().await?;

        // Use Redis sorted set to track requests in the current window
        let window_key = format!("{}:{}", key, window);

        // Atomic operations using Redis transaction
        let script = redis::Script::new(
            r#"
            local window_key = KEYS[1]
            local limit = tonumber(ARGV[1])
            local now = tonumber(ARGV[2])
            local window = tonumber(ARGV[3])
            
            -- Remove old entries
            redis.call('ZREMRANGEBYSCORE', window_key, '-inf', window - 2)
            
            -- Count requests in current window
            local count = redis.call('ZCARD', window_key)
            
            if count < limit then
                -- Add new request
                redis.call('ZADD', window_key, now, now)
                redis.call('EXPIRE', window_key, 180)  -- 3 minutes TTL
                return {1, limit - count - 1, window + 1}
            else
                return {0, 0, window + 1}
            end
            "#,
        );

        let result: Vec<i64> = script
            .key(&window_key)
            .arg(limit)
            .arg(now)
            .arg(window)
            .invoke_async(&mut conn)
            .await?;

        let allowed = result[0] == 1;
        let remaining = result[1] as u64;
        let reset_time = result[2] as u64 * 60; // Convert back to seconds

        // Update metrics
        self.metrics
            .requests_total
            .with_label_values(&[endpoint_type, username])
            .inc();

        if !allowed {
            self.metrics
                .rejections_total
                .with_label_values(&[endpoint_type, username])
                .inc();
        }

        // Update Redis health metric
        self.metrics.redis_health.set(1.0);

        Ok(RateLimitResult {
            allowed,
            limit,
            remaining,
            reset_time,
        })
    }

    fn get_limit_for_endpoint(&self, endpoint_type: &str) -> u64 {
        match endpoint_type {
            "search" => self.config.search_requests_per_minute,
            "chat" => self.config.chat_requests_per_minute,
            "transform" => self.config.transform_requests_per_minute,
            "test" => self.config.test_requests_per_minute,
            _ => self.config.default_requests_per_minute,
        }
    }
}

#[derive(Debug)]
pub struct RateLimitResult {
    pub allowed: bool,
    pub limit: u64,
    pub remaining: u64,
    pub reset_time: u64,
}

/// Middleware factory for rate limiting
pub struct RateLimitMiddleware {
    rate_limiter: Arc<RateLimiter>,
    enabled: bool,
}

impl RateLimitMiddleware {
    pub fn new(
        redis_client: ClusterClient,
        config: RateLimitConfig,
        metrics: RateLimitMetrics,
    ) -> Self {
        let enabled = config.enabled;
        let rate_limiter = Arc::new(RateLimiter::new(redis_client, config, metrics));
        Self {
            rate_limiter,
            enabled,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<BoxBody, B>>;
    type Error = Error;
    type Transform = RateLimitService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitService {
            service: Arc::new(service),
            rate_limiter: self.rate_limiter.clone(),
            enabled: self.enabled,
        }))
    }
}

/// Rate limiting service
pub struct RateLimitService<S> {
    service: Arc<S>,
    rate_limiter: Arc<RateLimiter>,
    enabled: bool,
}

impl<S, B> Service<ServiceRequest> for RateLimitService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<BoxBody, B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let rate_limiter = self.rate_limiter.clone();
        let enabled = self.enabled;

        Box::pin(async move {
            let path = req.path();

            // Skip rate limiting for health check and metrics endpoints
            if !path.starts_with("/api/") || !enabled {
                let res = service.call(req).await?;
                return Ok(res.map_into_right_body());
            }

            // Determine endpoint type from path
            let endpoint_type = determine_endpoint_type(path);

            // Extract username from request extensions (set by OIDC middleware)
            let username = req
                .extensions()
                .get::<crate::auth::AuthenticatedUser>()
                .map(|user| user.0.clone())
                .unwrap_or_else(|| "anonymous".to_string());

            // Check rate limit
            match rate_limiter
                .check_rate_limit(&username, endpoint_type)
                .await
            {
                Ok(result) => {
                    if result.allowed {
                        // Request allowed - call the service and add headers to response
                        let mut res = service.call(req).await?;
                        add_rate_limit_headers(&mut res, &result);
                        Ok(res.map_into_right_body())
                    } else {
                        // Rate limit exceeded
                        warn!(
                            username = %username,
                            endpoint_type = %endpoint_type,
                            limit = result.limit,
                            "Rate limit exceeded"
                        );

                        // Log to audit system
                        let mut audit_event = AuditEvent::new(
                            AuditEventType::RateLimitExceeded,
                            AuditOutcome::Denied,
                            &username,
                        )
                        .with_details(format!("endpoint: {}, limit: {}", path, result.limit));

                        if let Some(ip) = req.connection_info().peer_addr() {
                            audit_event = audit_event.with_client_ip(ip);
                        }

                        if let Some(request_id) = req.extensions().get::<uuid::Uuid>() {
                            audit_event = audit_event.with_request_id(request_id.to_string());
                        }

                        audit_event.log();

                        // Store in database asynchronously
                        if let Some(pool) = crate::audit::events::get_db_pool() {
                            tokio::spawn(async move {
                                if let Err(e) = audit_event.store(pool).await {
                                    warn!(
                                        target: "audit",
                                        error = %e,
                                        "Failed to store rate limit audit event in database"
                                    );
                                }
                            });
                        }

                        // Return 429 Too Many Requests
                        let retry_after = result.reset_time
                            - SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap()
                                .as_secs();

                        let response = HttpResponse::TooManyRequests()
                            .insert_header(("X-RateLimit-Limit", result.limit.to_string()))
                            .insert_header(("X-RateLimit-Remaining", "0"))
                            .insert_header(("X-RateLimit-Reset", result.reset_time.to_string()))
                            .insert_header(("Retry-After", retry_after.to_string()))
                            .json(serde_json::json!({
                                "error": "Rate limit exceeded",
                                "limit": result.limit,
                                "retry_after_seconds": retry_after,
                            }));

                        let (req_head, _) = req.into_parts();
                        let res = ServiceResponse::new(req_head, response);
                        Ok(res.map_into_left_body())
                    }
                }
                Err(e) => {
                    // Redis error - fail open (allow the request) but log the error
                    error!(error = %e, "Rate limiter Redis error, failing open");
                    rate_limiter.metrics.redis_health.set(0.0);
                    let res = service.call(req).await?;
                    Ok(res.map_into_right_body())
                }
            }
        })
    }
}

/// Add rate limit headers to response
fn add_rate_limit_headers<B>(res: &mut ServiceResponse<B>, result: &RateLimitResult) {
    res.headers_mut().insert(
        HeaderName::from_static("x-ratelimit-limit"),
        HeaderValue::from_str(&result.limit.to_string()).unwrap(),
    );
    res.headers_mut().insert(
        HeaderName::from_static("x-ratelimit-remaining"),
        HeaderValue::from_str(&result.remaining.to_string()).unwrap(),
    );
    res.headers_mut().insert(
        HeaderName::from_static("x-ratelimit-reset"),
        HeaderValue::from_str(&result.reset_time.to_string()).unwrap(),
    );
}

/// Determine endpoint type from request path for rate limiting
fn determine_endpoint_type(path: &str) -> &'static str {
    if path.contains("/search") || path.contains("/embedding") {
        "search"
    } else if path.contains("/chat") {
        "chat"
    } else if path.contains("/transforms") {
        "transform"
    } else if path.contains("/test") {
        "test"
    } else {
        "default"
    }
}
