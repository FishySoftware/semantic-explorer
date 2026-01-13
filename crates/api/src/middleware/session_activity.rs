//! Session activity tracking middleware
//!
//! Tracks user sessions and updates the last_activity timestamp for authenticated requests.
//! Sessions are created on-demand when an authenticated request is made without an existing session.
//! This middleware should be placed after OIDC authentication in the middleware stack.

use crate::auth::AuthenticatedUser;
use crate::storage::postgres::sessions;
use actix_web::{
    Error, HttpMessage,
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
};
use futures_util::future::{LocalBoxFuture, Ready, ready};
use sqlx::{Pool, Postgres};
use std::{
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};
use tracing::warn;

/// Session activity tracking middleware
pub struct SessionActivityMiddleware {
    pool: Arc<Pool<Postgres>>,
}

impl SessionActivityMiddleware {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for SessionActivityMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = SessionActivityService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(SessionActivityService {
            service: Rc::new(service),
            pool: self.pool.clone(),
        }))
    }
}

pub struct SessionActivityService<S> {
    service: Rc<S>,
    pool: Arc<Pool<Postgres>>,
}

impl<S, B> Service<ServiceRequest> for SessionActivityService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        let pool = self.pool.clone();

        // Extract request metadata before processing
        let username_opt = req
            .extensions()
            .get::<AuthenticatedUser>()
            .map(|user| user.0.clone());

        let ip_address = req.connection_info().peer_addr().map(|s| s.to_string());

        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());

        Box::pin(async move {
            // Process the request first
            let res = service.call(req).await?;

            // Track session activity in background (fire-and-forget)
            if let Some(username) = username_opt {
                let pool_clone = pool.clone();
                tokio::spawn(async move {
                    if let Err(e) = sessions::ensure_session_and_update_activity(
                        &pool_clone,
                        &username,
                        ip_address.as_deref(),
                        user_agent.as_deref(),
                    )
                    .await
                    {
                        warn!(error = %e, username = %username, "Failed to track session activity");
                    }
                });
            }

            Ok(res)
        })
    }
}
