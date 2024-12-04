use axum::{
    routing::{post, get},
    Router,
    http::{StatusCode, HeaderMap, Method, Request},
    response::Json,
    extract::State,
};
use rand::Rng;
use serde_json::json;
use std::sync::Arc;
use hyper_util::client::legacy::Client;
use hyper_tls::HttpsConnector;
use hyper_util::rt::TokioExecutor;
use http_body_util::{Full, BodyExt};
use dotenv::dotenv;
use serde_json::Value;
use chrono;
use std::time::Duration;
use std::env;
use bytes::Bytes;

// Configuration struct to hold environment variables
#[derive(Clone)]
struct Config {
    target_url: String,
    success_probability: f64,
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok();
        
        let target_url = env::var("TARGET_URL")
            .expect("TARGET_URL must be set");
            
        let success_probability = env::var("SUCCESS_PROBABILITY")
            .unwrap_or_else(|_| "0.8".to_string())
            .parse::<f64>()
            .expect("SUCCESS_PROBABILITY must be a float between 0.0 and 1.0");
            
        Config {
            target_url,
            success_probability,
        }
    }
}

// Shared HTTP client for proxying requests
type HttpClient = Client<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, Full<Bytes>>;
type SharedState = Arc<(HttpClient, Config)>;

#[tokio::main]
async fn main() {
    let config = Config::from_env();
    
    // Create HTTPS connector
    let https = HttpsConnector::new();
    let client = Client::builder(TokioExecutor::new())
        .build::<_, Full<Bytes>>(https);
    
    // Create shared state
    let state = Arc::new((client, config));

    let app = Router::new()
        .route("/delay", post(delay_handler))
        .route("/failure", post(failure_handler))
        .route("/healthcheck", get(healthcheck))
        .with_state(state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[axum::debug_handler]
async fn delay_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let (client, config) = &*state;
    
    // Parse delay configuration from headers
    let constant_delay_ms: Option<u64> = headers
        .get("X-Constant-Delay-Ms")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok());

    let max_random_delay_ms: Option<u64> = headers
        .get("X-Max-Random-Delay-Ms")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok());

    // Apply delays if specified
    if let Some(delay_ms) = constant_delay_ms {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }

    if let Some(max_delay_ms) = max_random_delay_ms {
        let random_delay = rand::thread_rng().gen_range(0..=max_delay_ms);
        tokio::time::sleep(Duration::from_millis(random_delay)).await;
    }

    // Allow header override of target URL for testing
    let target_url = headers
        .get("X-Proxy-Url")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(&config.target_url);

    // Convert the JSON payload to bytes
    let body_bytes = match serde_json::to_vec(&payload) {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Failed to serialize request body",
                    "details": e.to_string()
                }))
            );
        }
    };

    // Create and send the proxied request
    let req = Request::builder()
        .method(Method::POST)
        .uri(target_url)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body_bytes)))
        .unwrap();

    match client.request(req).await {
        Ok(resp) => {
            let status = resp.status();
            let body_bytes = match resp.into_body().collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({
                            "error": "Failed to read response body",
                            "details": e.to_string()
                        }))
                    );
                }
            };
            
            let body: Value = match serde_json::from_slice(&body_bytes) {
                Ok(json) => json,
                Err(_) => Value::Null,
            };
            
            (status, Json(json!({
                "status": "success",
                "applied_delays": {
                    "constant_delay_ms": constant_delay_ms,
                    "random_delay_ms": max_random_delay_ms.map(|max| format!("0-{}", max))
                },
                "target_url": target_url,
                "response": body
            })))
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to forward request",
                "details": e.to_string(),
                "target_url": target_url
            }))
        )
    }
}

// Add healthcheck handler
async fn healthcheck() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

#[axum::debug_handler]
async fn failure_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let (client, config) = &*state;
    
    // Check if we should return original response
    let return_original = headers
        .get("X-Return-Original")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(false);

    // Check for custom failure rate header
    let failure_rate: f64 = headers
        .get("X-Failure-Rate")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1.0 - config.success_probability);

    // Get custom failure status code from header, default to 500
    let failure_status = headers
        .get("X-Failure-Status-Code")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u16>().ok())
        .map(StatusCode::from_u16)
        .and_then(Result::ok)
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    // Generate random number before any await points
    let should_succeed = rand::thread_rng().gen_bool(1.0 - failure_rate);

    // Allow header override of target URL for testing
    let target_url = headers
        .get("X-Proxy-Url")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(&config.target_url);

    // If return_original is false, check if we should fail based on probability
    if !should_succeed {
        return (
            failure_status,
            Json(json!({
                "error": "Simulated failure",
                "target_url": target_url,
                "failure_rate": failure_rate,
                "status_code": failure_status.as_u16(),
                "request_body": payload
            }))
        );
    }

    // Convert the JSON payload to bytes
    let body_bytes = match serde_json::to_vec(&payload) {
        Ok(bytes) => bytes,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Failed to serialize request body",
                    "details": e.to_string()
                }))
            );
        }
    };

    // Create and send the proxied request
    let req = Request::builder()
        .method(Method::POST)
        .uri(target_url)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body_bytes)))
        .unwrap();

    match client.request(req).await {
        Ok(resp) => {
            let status = resp.status();
            let body_bytes = match resp.into_body().collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({
                            "error": "Failed to read response body",
                            "details": e.to_string()
                        }))
                    );
                }
            };
            
            let body: Value = match serde_json::from_slice(&body_bytes) {
                Ok(json) => json,
                Err(_) => Value::Null,
            };
            
            if return_original {
                (status, Json(body))
            } else {
                (status, Json(json!({
                    "status": "success",
                    "target_url": target_url,
                    "response": body
                })))
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "Failed to forward request",
                "details": e.to_string(),
                "target_url": target_url
            }))
        )
    }
}