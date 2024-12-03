use axum::{
    routing::{post, get},
    Router,
    http::{StatusCode, HeaderMap},
    response::Json,
    extract::State,
};
use rand::Rng;
use serde_json::json;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use hyper_tls::HttpsConnector;
use std::sync::Arc;
use bytes::Bytes as HyperBytes;
use http_body_util::{Full, BodyExt};
use std::env;
use dotenv::dotenv;
use serde_json::Value;
use chrono;

// Configuration struct to hold environment variables
#[derive(Clone)]
struct Config {
    target_url: String,
    success_probability: f64,
}

impl Config {
    fn from_env() -> Self {
        dotenv().ok(); // Load .env file if it exists
        
        let target_url = env::var("TARGET_URL")
            .expect("TARGET_URL must be set");
            
        let success_probability = env::var("SUCCESS_PROBABILITY")
            .unwrap_or_else(|_| "0.8".to_string()) // default to 0.8 if not set
            .parse::<f64>()
            .expect("SUCCESS_PROBABILITY must be a valid float between 0 and 1");
            
        if !(0.0..=1.0).contains(&success_probability) {
            panic!("SUCCESS_PROBABILITY must be between 0 and 1");
        }
        
        Self {
            target_url,
            success_probability,
        }
    }
}

// Shared HTTP client for proxying requests
type HttpClient = Client<HttpsConnector<hyper_util::client::legacy::connect::HttpConnector>, Full<HyperBytes>>;
type SharedState = Arc<(HttpClient, Config)>;

#[tokio::main]
async fn main() {
    // Load configuration from environment
    let config = Config::from_env();
    println!("Starting server with target URL: {}", config.target_url);
    println!("Success probability: {}", config.success_probability);
    
    // Create HTTPS connector
    let https = HttpsConnector::new();
    let client = Client::builder(TokioExecutor::new())
        .build::<_, Full<HyperBytes>>(https);
    
    // Create shared state
    let state = Arc::new((client, config));

    let app = Router::new()
        .route("/webhook", post(webhook_handler))
        .route("/proxy", post(proxy_handler))
        .route("/healthcheck", get(healthcheck))
        .with_state(state);
    
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

async fn webhook_handler(
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let mut rng = rand::thread_rng();
    let success_probability = 0.8;
    
    if rng.gen_bool(success_probability) {
        (StatusCode::OK, Json(json!({
            "status": "success",
            "message": "Webhook processed successfully",
            "received": payload
        })))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "status": "error",
            "message": "Internal server error"
        })))
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
async fn proxy_handler(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let (client, config) = &*state;
    
    // Generate random number before any await points
    let should_succeed = rand::thread_rng().gen_bool(config.success_probability);

    // Check if we should fail based on probability
    if !should_succeed {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Simulated failure",
                "target_url": &config.target_url,
                "success_probability": config.success_probability,
                "request_body": payload
            }))
        );
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
    
    // Prepare proxy request
    let request = match hyper::Request::builder()
        .method(hyper::Method::POST)
        .uri(target_url)
        .header("Content-Type", "application/json")
        .body(Full::new(HyperBytes::from(body_bytes))) {
            Ok(req) => req,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to build request",
                        "details": e.to_string(),
                        "target_url": target_url
                    }))
                );
            }
        };

    // Send proxied request
    match client.request(request).await {
        Ok(response) => {
            let status = response.status();
            match response.into_body().collect().await {
                Ok(collected) => {
                    let body = collected.to_bytes();
                    let body_str = String::from_utf8_lossy(&body).to_string();
                    
                    // Try to parse the response as JSON, fall back to string if not JSON
                    let response_body = serde_json::from_str::<Value>(&body_str)
                        .unwrap_or_else(|_| json!({"response": body_str}));
                    
                    (status, Json(response_body))
                },
                Err(e) => {
                    eprintln!("Failed to read response body: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": "Failed to read response body",
                            "details": e.to_string(),
                            "target_url": target_url,
                            "status_code": status.as_u16()
                        }))
                    )
                }
            }
        },
        Err(e) => {
            eprintln!("Proxy request failed: {}", e);
            eprintln!("Target URL: {}", target_url);
            eprintln!("Request body: {}", serde_json::to_string_pretty(&payload).unwrap());
            
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Proxy request failed",
                    "details": e.to_string(),
                    "target_url": target_url,
                    "request_body": payload
                }))
            )
        }
    }
}