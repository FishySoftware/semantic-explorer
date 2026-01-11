use actix_web::{
    Error, HttpMessage,
    dev::{ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{HeaderName, HeaderValue},
};
use futures_util::future::{LocalBoxFuture, Ready, ok};
use uuid::Uuid;

/// Request ID for tracing requests across services.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// Middleware that adds a unique request ID to each request
pub struct RequestIdMiddleware;

impl<S, B> Transform<S, ServiceRequest> for RequestIdMiddleware
where
    S: actix_web::dev::Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestIdMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdMiddlewareService { service })
    }
}

pub struct RequestIdMiddlewareService<S> {
    service: S,
}

impl<S, B> actix_web::dev::Service<ServiceRequest> for RequestIdMiddlewareService<S>
where
    S: actix_web::dev::Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>
        + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Check for existing request ID header, or generate a new one
        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Store request ID in request extensions for handler access
        req.extensions_mut().insert(RequestId(request_id.clone()));

        // Add request ID to tracing span
        let span = tracing::info_span!("request", request_id = %request_id);
        let _guard = span.enter();

        let fut = self.service.call(req);

        Box::pin(async move {
            let mut res = fut.await?;

            // Add request ID to response headers
            res.headers_mut().insert(
                HeaderName::from_static("x-request-id"),
                HeaderValue::from_str(&request_id)
                    .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
            );

            Ok(res)
        })
    }
}

/// Extract request ID from request extensions.
/// Part of Phase 4.3 - ready for handler integration.
#[allow(dead_code)] // Public API for handlers that need explicit request ID access
pub fn get_request_id(req: &actix_web::HttpRequest) -> Option<String> {
    req.extensions().get::<RequestId>().map(|r| r.0.clone())
}
