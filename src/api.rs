//! REST API server

use std::net::SocketAddr;
use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::{scanners, types::*};

#[derive(Debug, Deserialize)]
struct ScanQuery {
    validator: String,
    program: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    version: &'static str,
}

#[derive(Debug, Serialize)]
struct ProgramInfo {
    name: &'static str,
    display_name: &'static str,
    description: &'static str,
    registration_url: &'static str,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

pub async fn serve(host: &str, port: u16) -> anyhow::Result<()> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/programs", get(programs))
        .route("/api/scan", get(scan))
        .layer(cors);
    
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    tracing::info!("Starting Delegation Oracle API on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn index() -> &'static str {
    "Delegation Oracle API - https://github.com/jque-designs/delegation-oracle"
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn programs() -> Json<Vec<ProgramInfo>> {
    Json(vec![
        ProgramInfo {
            name: "marinade",
            display_name: "Marinade Finance",
            description: "Native stake + mSOL LST, MNDE rewards",
            registration_url: "https://marinade.finance/validators",
        },
        ProgramInfo {
            name: "jito",
            display_name: "Jito StakeNet",
            description: "jitoSOL LST + MEV rewards sharing",
            registration_url: "https://jito.network/stakenet",
        },
        ProgramInfo {
            name: "blaze",
            display_name: "SolBlaze",
            description: "bSOL LST + BLZE token rewards",
            registration_url: "https://stake.solblaze.org",
        },
        ProgramInfo {
            name: "sanctum",
            display_name: "Sanctum Gauge",
            description: "vSOL gauge voting for stake allocation",
            registration_url: "https://app.sanctum.so",
        },
        ProgramInfo {
            name: "sfdp",
            display_name: "Solana Foundation",
            description: "Foundation Delegation Program",
            registration_url: "https://solana.org/delegation-program",
        },
    ])
}

async fn scan(
    Query(query): Query<ScanQuery>,
) -> Result<Json<ScanResult>, (StatusCode, Json<ErrorResponse>)> {
    // Validate pubkey format (basic check)
    if query.validator.len() < 32 || query.validator.len() > 44 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid validator pubkey".to_string(),
            }),
        ));
    }
    
    match scanners::scan_validator(&query.validator, query.program.as_deref()).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}
