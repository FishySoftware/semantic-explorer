use actix_web::{
    HttpMessage, HttpResponse,
    body::{BoxBody, EitherBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::HeaderValue,
};
use anyhow::Result;
use futures_util::future::LocalBoxFuture;
use redis::AsyncCommands;
use semantic_explorer_core::observability::{increment_counter, record_histogram};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    future::{Ready, ready},
    sync::Arc,
    time::Instant,
};
use tracing::{debug, info, warn};

const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
const IDEMPOTENCY_TTL_SECS: u64 = 86400; // 24 hours

/// Stored idempotent response
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IdempotentResponse {
    status: u16,
    headers: HashMap<String, String>,
    body: String,
    timestamp: i64,
}

/// Idempotency middleware configuration
#[derive(Clone)]
pub struct IdempotencyConfig {
    pub enabled: bool,
    pub redis_client: redis::cluster::ClusterClient,
    pub key_prefix: String,
    pub ttl_secs: u64,
}

impl IdempotencyConfig {
    pub fn new(redis_client: redis::cluster::ClusterClient) -> Self {
        Self {
            enabled: true,
            redis_client,
            key_prefix: "idempotency".to_string(),
            ttl_secs: IDEMPOTENCY_TTL_SECS,
        }
    }

    pub fn with_prefix(mut self, prefix: String) -> Self {
        self.key_prefix = prefix;
        self
    }

    pub fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs;
        self
    }
}

/// Idempotency middleware
pub struct IdempotencyMiddleware {
    config: Arc<IdempotencyConfig>,
}

impl IdempotencyMiddleware {
    pub fn new(config: IdempotencyConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Check if the request should be subject to idempotency checks
    fn should_check_idempotency(req: &ServiceRequest) -> bool {
        let path = req.path();
        let method = req.method();

        // Only apply idempotency to POST requests on transform trigger endpoints
        if !method.as_str().eq_ignore_ascii_case("POST") {
            return false;
        }

        // Check if path matches transform trigger endpoints
        path.contains("/transforms/") && path.ends_with("/trigger")
    }

    /// Extract username from authenticated request
    fn extract_username(req: &ServiceRequest) -> Option<String> {
        req.request()
            .extensions()
            .get::<crate::auth::AuthenticatedUser>()
            .map(|u| u.0.clone())
    }

    /// Build Redis key for idempotency
    fn build_redis_key(
        config: &IdempotencyConfig,
        username: &str,
        idempotency_key: &str,
        endpoint: &str,
    ) -> String {
        format!(
            "{}:{}:{}:{}",
            config.key_prefix, username, idempotency_key, endpoint
        )
    }
}

impl<S, B> Transform<S, ServiceRequest> for IdempotencyMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<BoxBody, B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = IdempotencyMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(IdempotencyMiddlewareService {
            service: Arc::new(service),
            config: self.config.clone(),
        }))
    }
}

pub struct IdempotencyMiddlewareService<S> {
    service: Arc<S>,
    config: Arc<IdempotencyConfig>,
}

impl<S, B> Service<ServiceRequest> for IdempotencyMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<EitherBody<BoxBody, B>>;
    type Error = actix_web::Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start = Instant::now();

        // Check if idempotency is disabled
        if !self.config.enabled {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_right_body())
            });
        }

        // Check if this endpoint requires idempotency
        if !IdempotencyMiddleware::should_check_idempotency(&req) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_right_body())
            });
        }

        // Extract idempotency key from header
        let idempotency_key = match req.headers().get(IDEMPOTENCY_KEY_HEADER) {
            Some(key) => match key.to_str() {
                Ok(k) => k.to_string(),
                Err(_) => {
                    debug!("Invalid idempotency key header value");
                    let fut = self.service.call(req);
                    return Box::pin(async move {
                        let res = fut.await?;
                        Ok(res.map_into_right_body())
                    });
                }
            },
            None => {
                // No idempotency key provided - process normally
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_right_body())
                });
            }
        };

        // Extract username
        let username = match IdempotencyMiddleware::extract_username(&req) {
            Some(u) => u,
            None => {
                warn!("Cannot apply idempotency without authenticated user");
                let fut = self.service.call(req);
                return Box::pin(async move {
                    let res = fut.await?;
                    Ok(res.map_into_right_body())
                });
            }
        };

        let endpoint = req.path().to_string();
        let config = self.config.clone();
        let service = self.service.clone();

        Box::pin(async move {
            // Build Redis key
            let redis_key = IdempotencyMiddleware::build_redis_key(
                &config,
                &username,
                &idempotency_key,
                &endpoint,
            );

            // Try to get existing response from Redis
            let mut conn = match config.redis_client.get_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "Failed to connect to Redis for idempotency check - continuing without idempotency");
                    increment_counter("idempotency.redis_errors", &[("operation", "connect")]);
                    let res = service.call(req).await?;
                    return Ok(res.map_into_right_body());
                }
            };

            // Check if we have a cached response
            match conn.get::<_, String>(&redis_key).await {
                Ok(cached_json) => {
                    // Found a cached response - deserialize and return it
                    match serde_json::from_str::<IdempotentResponse>(&cached_json) {
                        Ok(cached_resp) => {
                            info!(
                                username = %username,
                                idempotency_key = %idempotency_key,
                                endpoint = %endpoint,
                                "Returning cached idempotent response"
                            );

                            let duration = start.elapsed().as_secs_f64();
                            record_histogram(
                                "idempotency.request_duration",
                                duration,
                                &[("cache", "hit")],
                            );
                            increment_counter("idempotency.cache_hits", &[("endpoint", &endpoint)]);

                            // Build response from cached data
                            let mut response_builder = HttpResponse::build(
                                actix_web::http::StatusCode::from_u16(cached_resp.status)
                                    .unwrap_or(actix_web::http::StatusCode::OK),
                            );

                            // Add cached headers
                            for (key, value) in &cached_resp.headers {
                                if let Ok(header_value) = HeaderValue::from_str(value) {
                                    response_builder.append_header((key.as_str(), header_value));
                                }
                            }

                            // Add idempotency replay header
                            response_builder.append_header(("X-Idempotency-Replay", "true"));

                            // Include the cached body
                            let response = response_builder.body(cached_resp.body);
                            let (req_head, _) = req.into_parts();
                            let res = ServiceResponse::new(req_head, response);
                            return Ok(res.map_into_left_body());
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to deserialize cached idempotent response");
                            increment_counter("idempotency.deserialization_errors", &[]);
                            // Continue with normal processing if deserialization fails
                        }
                    }
                }
                Err(e) if e.kind() == redis::ErrorKind::UnexpectedReturnType => {
                    // Key doesn't exist (Redis returns type error for non-existent keys when trying to deserialize)
                    debug!(
                        username = %username,
                        idempotency_key = %idempotency_key,
                        "No cached response found - processing request"
                    );
                }
                Err(e) => {
                    warn!(error = %e, "Failed to check Redis for idempotent response");
                    increment_counter("idempotency.redis_errors", &[("operation", "get")]);
                }
            }

            // No cached response - process the request
            increment_counter("idempotency.cache_misses", &[("endpoint", &endpoint)]);

            let response = service.call(req).await?;

            // Cache successful responses (2xx status codes)
            if response.status().is_success() {
                let status_code = response.status();

                // Collect headers from response
                let mut headers_map = HashMap::new();
                for (key, value) in response.headers().iter() {
                    if let Ok(value_str) = value.to_str() {
                        headers_map.insert(key.as_str().to_string(), value_str.to_string());
                    }
                }

                // NOTE: Body capture for idempotency is complex in actix-web due to streaming bodies.
                // For now, we cache a marker response with status/headers but empty body.
                // This still prevents duplicate transform operations since the cached status indicates success.
                // Full body caching would require buffering the entire response, which could be large.
                // FUTURE: Implement full body capture with custom MessageBody wrapper and size limits
                //         See REFACTORING_SUMMARY.md for detailed implementation path

                let cached_response = IdempotentResponse {
                    status: status_code.as_u16(),
                    headers: headers_map,
                    body: "".to_string(), // Body capture deferred (see NOTE above)
                    timestamp: chrono::Utc::now().timestamp(),
                };

                if let Ok(cached_json) = serde_json::to_string(&cached_response) {
                    let redis_key_clone = redis_key.clone();
                    let ttl = config.ttl_secs;

                    // Store in Redis asynchronously (don't wait for it)
                    tokio::spawn(async move {
                        let mut conn = match config.redis_client.get_async_connection().await {
                            Ok(c) => c,
                            Err(e) => {
                                warn!(error = %e, "Failed to connect to Redis for caching response");
                                return;
                            }
                        };

                        if let Err(e) = conn
                            .set_ex::<_, _, ()>(&redis_key_clone, cached_json, ttl)
                            .await
                        {
                            warn!(error = %e, "Failed to cache idempotent response in Redis");
                            increment_counter("idempotency.redis_errors", &[("operation", "set")]);
                        } else {
                            info!(
                                redis_key = %redis_key_clone,
                                ttl_secs = %ttl,
                                "Cached idempotent response (status and headers only)"
                            );
                        }
                    });
                }
            }

            let duration = start.elapsed().as_secs_f64();
            record_histogram(
                "idempotency.request_duration",
                duration,
                &[("cache", "miss")],
            );

            Ok(response.map_into_right_body())
        })
    }
}
