use actix_web::{HttpResponse, Responder, get, web::Data};
use aws_sdk_s3::Client as S3Client;
use qdrant_client::Qdrant;
use semantic_explorer_core::config::S3Config;
use serde::Serialize;
use sqlx::{Pool, Postgres};
use std::time::Instant;

/// Health check status for individual components
#[derive(Debug, Clone, Serialize)]
pub struct ComponentHealth {
    pub status: HealthStatus,
    pub latency_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Unhealthy,
    Degraded,
}

/// Full health response with all component statuses
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: &'static str,
    pub components: ComponentsHealth,
}

#[derive(Debug, Serialize)]
pub struct ComponentsHealth {
    pub postgres: ComponentHealth,
    pub qdrant: ComponentHealth,
    pub s3: ComponentHealth,
    pub nats: ComponentHealth,
}

impl HealthResponse {
    fn overall_status(components: &ComponentsHealth) -> HealthStatus {
        let statuses = [
            &components.postgres.status,
            &components.qdrant.status,
            &components.s3.status,
            &components.nats.status,
        ];

        if statuses.iter().all(|s| matches!(s, HealthStatus::Healthy)) {
            HealthStatus::Healthy
        } else if statuses
            .iter()
            .any(|s| matches!(s, HealthStatus::Unhealthy))
        {
            HealthStatus::Unhealthy
        } else {
            HealthStatus::Degraded
        }
    }
}

#[get("/health/live")]
pub async fn liveness() -> impl Responder {
    HttpResponse::Ok().body("ok")
}

#[get("/health/ready")]
pub async fn readiness(
    pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    s3_client: Data<S3Client>,
    s3_config: Data<S3Config>,
    nats_client: Data<async_nats::Client>,
) -> impl Responder {
    let (postgres_health, qdrant_health, s3_health, nats_health) = tokio::join!(
        check_postgres(&pool),
        check_qdrant(&qdrant_client),
        check_s3(&s3_client, &s3_config.bucket_name),
        check_nats(&nats_client),
    );

    let components = ComponentsHealth {
        postgres: postgres_health,
        qdrant: qdrant_health,
        s3: s3_health,
        nats: nats_health,
    };

    let status = HealthResponse::overall_status(&components);
    let response = HealthResponse {
        status: status.clone(),
        version: env!("CARGO_PKG_VERSION"),
        components,
    };

    match status {
        HealthStatus::Healthy => HttpResponse::Ok().json(response),
        HealthStatus::Degraded => HttpResponse::Ok().json(response),
        HealthStatus::Unhealthy => HttpResponse::ServiceUnavailable().json(response),
    }
}

async fn check_postgres(pool: &Pool<Postgres>) -> ComponentHealth {
    let start = Instant::now();

    match sqlx::query("SELECT 1").fetch_one(pool).await {
        Ok(_) => ComponentHealth {
            status: HealthStatus::Healthy,
            latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
            error: None,
        },
        Err(e) => {
            // Log detailed error for debugging, but don't expose to client
            tracing::error!(error = %e, "Database health check failed");
            ComponentHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
                error: Some("Database connection failed".to_string()),
            }
        }
    }
}

async fn check_qdrant(client: &Qdrant) -> ComponentHealth {
    let start = Instant::now();

    match client.health_check().await {
        Ok(_) => ComponentHealth {
            status: HealthStatus::Healthy,
            latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
            error: None,
        },
        Err(e) => {
            tracing::error!(error = %e, "Qdrant health check failed");
            ComponentHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
                error: Some("Vector database connection failed".to_string()),
            }
        }
    }
}

async fn check_s3(client: &S3Client, bucket_name: &str) -> ComponentHealth {
    let start = Instant::now();
    match client.head_bucket().bucket(bucket_name).send().await {
        Ok(_) => ComponentHealth {
            status: HealthStatus::Healthy,
            latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
            error: None,
        },
        Err(e) => {
            tracing::error!(error = %e, "S3 health check failed");
            ComponentHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_secs_f64() * 1000.0),
                error: Some("Object storage connection failed".to_string()),
            }
        }
    }
}

async fn check_nats(client: &async_nats::Client) -> ComponentHealth {
    let start = Instant::now();
    let state = client.connection_state();
    let latency = start.elapsed().as_secs_f64() * 1000.0;

    match state {
        async_nats::connection::State::Connected => ComponentHealth {
            status: HealthStatus::Healthy,
            latency_ms: Some(latency),
            error: None,
        },
        async_nats::connection::State::Disconnected => ComponentHealth {
            status: HealthStatus::Unhealthy,
            latency_ms: Some(latency),
            error: Some("NATS disconnected".to_string()),
        },
        async_nats::connection::State::Pending => ComponentHealth {
            status: HealthStatus::Degraded,
            latency_ms: Some(latency),
            error: Some("NATS connection pending".to_string()),
        },
    }
}
