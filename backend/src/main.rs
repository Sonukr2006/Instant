use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    http::{header::AUTHORIZATION, request::Parts, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, RequestPartsExt, Router,
};
use chrono::Utc;
use dashmap::DashMap;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com";
const DEFAULT_GEMINI_API_VERSION: &str = "v1beta";
const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8080";
const DEFAULT_FREE_DAILY_LIMIT: u32 = 20;
const DEFAULT_PRO_DAILY_LIMIT: u32 = 500;
const DEFAULT_MAX_PROMPT_CHARS: usize = 60_000;
const MAX_ERROR_DETAIL_CHARS: usize = 1_500;
const MIN_JWT_SECRET_BYTES: usize = 32;
const AI_SYSTEM_PROMPT: &str = "You are a smart, adaptive developer assistant.
Analyze the incoming user payload before choosing a response style.
If the payload is a casual message, greeting, reading note, PDF excerpt, or general conceptual question, respond naturally, conversationally, and concisely as a helpful peer.
Only if the payload contains an explicit code snippet, structural data configuration, command output, or software error stack trace, activate a strict, professional diagnostic format.
In diagnostic mode, prioritize bugs, risks, correctness, security, performance, maintainability, and developer experience.
When refactoring is useful, provide optimized code with concise rationale and avoid vague advice.";

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    config: Arc<AppConfig>,
    usage: Arc<DashMap<UsageKey, u32>>,
}

#[derive(Clone)]
struct AppConfig {
    bind_addr: SocketAddr,
    jwt_secret: String,
    jwt_issuer: Option<String>,
    jwt_audience: Option<String>,
    gemini_api_key: String,
    gemini_model: String,
    gemini_api_version: String,
    free_daily_request_limit: u32,
    pro_daily_request_limit: u32,
    max_prompt_chars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UsageKey {
    user_id: String,
    day: String,
}

#[derive(Debug, Deserialize)]
struct AskRequest {
    prompt_context: String,
}

#[derive(Debug, Serialize)]
struct AskResponse {
    response_text: String,
    remaining_requests_today: u32,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    #[serde(rename = "exp")]
    _exp: usize,
    #[serde(rename = "iss")]
    _iss: Option<String>,
    #[serde(rename = "aud")]
    _aud: Option<String>,
    #[serde(rename = "jti")]
    _jti: Option<String>,
    plan: Option<UserPlan>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum UserPlan {
    Free,
    Pro,
}

#[derive(Debug, Clone)]
struct AuthUser {
    id: String,
    plan: UserPlan,
}

#[derive(Debug, Clone)]
struct QuotaCharge {
    key: UsageKey,
    remaining_requests_today: u32,
}

#[derive(Debug)]
enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    TooManyRequests(String),
    BadGateway(String),
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiRequestContent>,
}

#[derive(Serialize)]
struct GeminiRequestContent {
    parts: Vec<GeminiRequestPart>,
}

#[derive(Serialize)]
struct GeminiRequestPart {
    text: String,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "promptFeedback")]
    prompt_feedback: Option<GeminiPromptFeedback>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiResponseContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct GeminiPromptFeedback {
    #[serde(rename = "blockReason")]
    block_reason: Option<String>,
}

#[derive(Deserialize)]
struct GeminiResponseContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), ApiError> {
    dotenvy::dotenv().ok();
    init_tracing();

    let config = Arc::new(AppConfig::from_env()?);
    let state = AppState {
        client: reqwest::Client::builder()
            .timeout(Duration::from_secs(35))
            .build()
            .map_err(|error| {
                ApiError::Internal(format!("Failed to initialize HTTP client: {error}"))
            })?,
        config: config.clone(),
        usage: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/ai/ask", post(ask_ai))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .map_err(|error| ApiError::Internal(format!("Failed to bind server: {error}")))?;

    tracing::info!("Instant AI Context API listening on {}", config.bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|error| ApiError::Internal(format!("Server failed: {error}")))?;

    Ok(())
}

impl AppConfig {
    fn from_env() -> Result<Self, ApiError> {
        let bind_addr = read_env_or_default("BIND_ADDR", DEFAULT_BIND_ADDR)
            .parse::<SocketAddr>()
            .map_err(|error| ApiError::Internal(format!("Invalid BIND_ADDR: {error}")))?;
        let jwt_secret = required_secret_env("JWT_SECRET")?;
        let gemini_api_key = required_env("GEMINI_API_KEY")?;

        Ok(Self {
            bind_addr,
            jwt_secret,
            jwt_issuer: read_optional_env("JWT_ISSUER"),
            jwt_audience: read_optional_env("JWT_AUDIENCE"),
            gemini_api_key,
            gemini_model: read_env_or_default("GEMINI_MODEL", DEFAULT_GEMINI_MODEL),
            gemini_api_version: read_env_or_default(
                "GEMINI_API_VERSION",
                DEFAULT_GEMINI_API_VERSION,
            ),
            free_daily_request_limit: read_env_or_default(
                "FREE_DAILY_REQUEST_LIMIT",
                &DEFAULT_FREE_DAILY_LIMIT.to_string(),
            )
            .parse()
            .map_err(|error| {
                ApiError::Internal(format!("Invalid FREE_DAILY_REQUEST_LIMIT: {error}"))
            })?,
            pro_daily_request_limit: read_env_or_default(
                "PRO_DAILY_REQUEST_LIMIT",
                &DEFAULT_PRO_DAILY_LIMIT.to_string(),
            )
            .parse()
            .map_err(|error| {
                ApiError::Internal(format!("Invalid PRO_DAILY_REQUEST_LIMIT: {error}"))
            })?,
            max_prompt_chars: read_env_or_default(
                "MAX_PROMPT_CHARS",
                &DEFAULT_MAX_PROMPT_CHARS.to_string(),
            )
            .parse()
            .map_err(|error| ApiError::Internal(format!("Invalid MAX_PROMPT_CHARS: {error}")))?,
        })
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn ask_ai(
    State(state): State<AppState>,
    user: AuthUser,
    Json(payload): Json<AskRequest>,
) -> Result<Json<AskResponse>, ApiError> {
    let prompt_context = payload.prompt_context.trim();

    if prompt_context.is_empty() {
        return Err(ApiError::BadRequest(
            "Cannot request an AI response without text context.".to_string(),
        ));
    }

    let prompt_chars = prompt_context.chars().count();
    if prompt_chars > state.config.max_prompt_chars {
        return Err(ApiError::BadRequest(format!(
            "Context is too large. Maximum allowed: {} characters. Current size: {prompt_chars}.",
            state.config.max_prompt_chars
        )));
    }

    let quota_charge = consume_quota(&state, &user)?;
    let response_text = match fetch_gemini_response(&state, prompt_context).await {
        Ok(response_text) => response_text,
        Err(error) => {
            refund_quota(&state, &quota_charge.key);
            return Err(error);
        }
    };

    Ok(Json(AskResponse {
        response_text,
        remaining_requests_today: quota_charge.remaining_requests_today,
    }))
}

async fn fetch_gemini_response(state: &AppState, prompt_context: &str) -> Result<String, ApiError> {
    let model = state.config.gemini_model.trim_start_matches("models/");
    let payload = GeminiRequest {
        contents: vec![GeminiRequestContent {
            parts: vec![GeminiRequestPart {
                text: format!("{AI_SYSTEM_PROMPT}\n\nContext:\n{prompt_context}"),
            }],
        }],
    };

    let response = state
        .client
        .post(format!(
            "{}/{}/models/{model}:generateContent",
            GEMINI_API_BASE_URL, state.config.gemini_api_version
        ))
        .query(&[("key", state.config.gemini_api_key.as_str())])
        .json(&payload)
        .send()
        .await
        .map_err(|error| ApiError::BadGateway(format!("Failed to reach Gemini API: {error}")))?;

    let status = response.status();

    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        let detail = truncate_error_detail(detail.trim());

        return Err(ApiError::BadGateway(format!(
            "Gemini API request failed with HTTP {}. {}",
            status.as_u16(),
            detail
        )));
    }

    let data = response.json::<GeminiResponse>().await.map_err(|error| {
        ApiError::BadGateway(format!("Failed to parse Gemini response: {error}"))
    })?;

    extract_gemini_text(data)
}

fn extract_gemini_text(data: GeminiResponse) -> Result<String, ApiError> {
    let block_reason = data
        .prompt_feedback
        .and_then(|feedback| feedback.block_reason)
        .filter(|reason| !reason.trim().is_empty());

    let Some(candidate) = data
        .candidates
        .and_then(|candidates| candidates.into_iter().next())
    else {
        return Err(gemini_empty_response_error(block_reason, None));
    };

    let finish_reason = candidate
        .finish_reason
        .filter(|reason| !reason.trim().is_empty());
    let text = candidate
        .content
        .and_then(|content| content.parts)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|part| part.text)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        Err(gemini_empty_response_error(block_reason, finish_reason))
    } else {
        Ok(text)
    }
}

fn gemini_empty_response_error(
    block_reason: Option<String>,
    finish_reason: Option<String>,
) -> ApiError {
    if let Some(block_reason) = block_reason {
        return ApiError::BadGateway(format!(
            "Gemini blocked the prompt before generation. Reason: {block_reason}."
        ));
    }

    if let Some(finish_reason) = finish_reason {
        return ApiError::BadGateway(format!(
            "Gemini finished without generated text. Reason: {finish_reason}."
        ));
    }

    ApiError::BadGateway("Gemini response did not contain generated text.".to_string())
}

fn consume_quota(state: &AppState, user: &AuthUser) -> Result<QuotaCharge, ApiError> {
    let limit = match user.plan {
        UserPlan::Free => state.config.free_daily_request_limit,
        UserPlan::Pro => state.config.pro_daily_request_limit,
    };
    let day = Utc::now().format("%Y-%m-%d").to_string();
    cleanup_stale_usage(state, &day);

    let key = UsageKey {
        user_id: user.id.clone(),
        day,
    };
    let mut used = state.usage.entry(key).or_insert(0);

    if *used >= limit {
        return Err(ApiError::TooManyRequests(format!(
            "Daily request limit reached for your plan. Limit: {limit}."
        )));
    }

    *used += 1;
    Ok(QuotaCharge {
        key: used.key().clone(),
        remaining_requests_today: limit.saturating_sub(*used),
    })
}

fn refund_quota(state: &AppState, key: &UsageKey) {
    if let Some(mut used) = state.usage.get_mut(key) {
        *used = used.saturating_sub(1);

        if *used == 0 {
            drop(used);
            state.usage.remove(key);
        }
    }
}

fn cleanup_stale_usage(state: &AppState, current_day: &str) {
    state.usage.retain(|key, _| key.day == current_day);
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let State(state): State<AppState> = parts
            .extract_with_state(state)
            .await
            .map_err(|_| ApiError::Internal("Failed to resolve app state.".to_string()))?;

        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::Unauthorized("Missing bearer token.".to_string()))?;

        let claims = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &jwt_validation(&state.config),
        )
        .map_err(|_| ApiError::Unauthorized("Invalid or expired bearer token.".to_string()))?
        .claims;

        Ok(AuthUser {
            id: claims.sub,
            plan: claims.plan.unwrap_or(UserPlan::Free),
        })
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            ApiError::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            ApiError::Unauthorized(message) => (StatusCode::UNAUTHORIZED, message),
            ApiError::TooManyRequests(message) => (StatusCode::TOO_MANY_REQUESTS, message),
            ApiError::BadGateway(message) => (StatusCode::BAD_GATEWAY, message),
            ApiError::Internal(message) => {
                tracing::error!("{message}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error.".to_string(),
                )
            }
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(HeaderValue::from_static("tauri://localhost"))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([AUTHORIZATION, axum::http::header::CONTENT_TYPE])
}

fn required_env(key: &str) -> Result<String, ApiError> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::Internal(format!("Missing required environment variable: {key}")))
}

fn required_secret_env(key: &str) -> Result<String, ApiError> {
    let value = required_env(key)?;
    validate_jwt_secret(key, &value)?;
    Ok(value)
}

fn validate_jwt_secret(key: &str, value: &str) -> Result<(), ApiError> {
    let normalized = value.trim().to_ascii_lowercase();
    let weak_placeholders = [
        "replace_with_a_long_random_secret",
        "change_me",
        "changeme",
        "secret",
        "password",
    ];

    if value.len() < MIN_JWT_SECRET_BYTES {
        return Err(ApiError::Internal(format!(
            "{key} must be at least {MIN_JWT_SECRET_BYTES} bytes long."
        )));
    }

    if weak_placeholders.contains(&normalized.as_str()) || normalized.starts_with("replace_") {
        return Err(ApiError::Internal(format!(
            "{key} is still set to an unsafe placeholder value."
        )));
    }

    Ok(())
}

fn read_optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn jwt_validation(config: &AppConfig) -> Validation {
    let mut validation = Validation::new(Algorithm::HS256);

    if let Some(issuer) = &config.jwt_issuer {
        validation.set_issuer(&[issuer]);
        validation.set_required_spec_claims(&["exp", "iss"]);
    }

    if let Some(audience) = &config.jwt_audience {
        validation.set_audience(&[audience]);
        validation.set_required_spec_claims(if config.jwt_issuer.is_some() {
            &["exp", "iss", "aud"]
        } else {
            &["exp", "aud"]
        });
    }

    validation
}

fn truncate_error_detail(detail: &str) -> String {
    let mut chars = detail.chars();
    let truncated: String = chars.by_ref().take(MAX_ERROR_DETAIL_CHARS).collect();

    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install terminate signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state(free_limit: u32, pro_limit: u32) -> AppState {
        AppState {
            client: reqwest::Client::new(),
            config: Arc::new(AppConfig {
                bind_addr: DEFAULT_BIND_ADDR.parse().expect("valid bind addr"),
                jwt_secret: "test-secret-with-at-least-32-characters".to_string(),
                jwt_issuer: None,
                jwt_audience: None,
                gemini_api_key: "test-api-key".to_string(),
                gemini_model: DEFAULT_GEMINI_MODEL.to_string(),
                gemini_api_version: DEFAULT_GEMINI_API_VERSION.to_string(),
                free_daily_request_limit: free_limit,
                pro_daily_request_limit: pro_limit,
                max_prompt_chars: DEFAULT_MAX_PROMPT_CHARS,
            }),
            usage: Arc::new(DashMap::new()),
        }
    }

    fn free_user() -> AuthUser {
        AuthUser {
            id: "user-1".to_string(),
            plan: UserPlan::Free,
        }
    }

    #[test]
    fn quota_rejects_after_plan_limit() {
        let state = test_state(1, 10);
        let user = free_user();

        let charge = consume_quota(&state, &user).expect("first request should pass");
        assert_eq!(charge.remaining_requests_today, 0);

        let error = consume_quota(&state, &user).expect_err("second request should fail");
        assert!(matches!(error, ApiError::TooManyRequests(_)));
    }

    #[test]
    fn quota_refund_allows_retry_after_upstream_failure() {
        let state = test_state(1, 10);
        let user = free_user();

        let charge = consume_quota(&state, &user).expect("first request should pass");
        refund_quota(&state, &charge.key);

        let retry = consume_quota(&state, &user).expect("refunded request should be reusable");
        assert_eq!(retry.remaining_requests_today, 0);
    }

    #[test]
    fn stale_usage_entries_are_cleaned_on_consume() {
        let state = test_state(2, 10);
        state.usage.insert(
            UsageKey {
                user_id: "user-1".to_string(),
                day: "2000-01-01".to_string(),
            },
            1,
        );

        consume_quota(&state, &free_user()).expect("current request should pass");

        assert_eq!(state.usage.len(), 1);
        assert!(state
            .usage
            .iter()
            .all(|entry| entry.key().day != "2000-01-01"));
    }

    #[test]
    fn gemini_text_parts_are_joined() {
        let response = GeminiResponse {
            candidates: Some(vec![GeminiCandidate {
                content: Some(GeminiResponseContent {
                    parts: Some(vec![
                        GeminiResponsePart {
                            text: Some("first".to_string()),
                        },
                        GeminiResponsePart {
                            text: Some("second".to_string()),
                        },
                    ]),
                }),
                finish_reason: Some("STOP".to_string()),
            }]),
            prompt_feedback: None,
        };

        assert_eq!(
            extract_gemini_text(response).expect("text should parse"),
            "first\nsecond"
        );
    }

    #[test]
    fn gemini_block_reason_is_reported() {
        let response = GeminiResponse {
            candidates: None,
            prompt_feedback: Some(GeminiPromptFeedback {
                block_reason: Some("SAFETY".to_string()),
            }),
        };

        let error = extract_gemini_text(response).expect_err("blocked response should fail");
        match error {
            ApiError::BadGateway(message) => assert!(message.contains("SAFETY")),
            _ => panic!("expected bad gateway"),
        }
    }
}
