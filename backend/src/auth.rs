use std::collections::HashMap;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;

const MIN_JWT_SECRET_BYTES: usize = 32;

#[derive(Clone)]
pub struct JwtConfig {
    pub secret: String,
    pub issuer: Option<String>,
    pub audience: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UserPlan {
    Free,
    Pro,
}

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: String,
    pub plan: UserPlan,
}

#[derive(Clone)]
pub struct UserDirectory {
    mode: UserDirectoryMode,
}

#[derive(Clone)]
enum UserDirectoryMode {
    Static(HashMap<String, UserRecord>),
    JwtClaimsDevelopment,
}

#[derive(Clone)]
struct UserRecord {
    plan: UserPlan,
    active: bool,
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

#[derive(Debug)]
pub enum AuthError {
    DisabledUser,
    InvalidToken,
    MissingBearerToken,
    UnknownUser,
}

impl AuthError {
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::DisabledUser => "User session is disabled.",
            Self::InvalidToken => "Invalid or expired bearer token.",
            Self::MissingBearerToken => "Missing bearer token.",
            Self::UnknownUser => "User session is not recognized.",
        }
    }
}

impl UserDirectory {
    pub fn from_static_users(value: &str) -> Result<Self, String> {
        Ok(Self {
            mode: UserDirectoryMode::Static(parse_static_users(value)?),
        })
    }

    pub fn from_env() -> Result<Self, String> {
        match read_env_or_default("AUTH_USER_DIRECTORY", "static").as_str() {
            "static" => {
                let users = required_env("AUTH_STATIC_USERS")?;
                Self::from_static_users(&users)
            }
            "jwt_claims_dev" => {
                if !env_flag_enabled("ALLOW_DEV_JWT_AUTH") {
                    return Err(
                        "AUTH_USER_DIRECTORY=jwt_claims_dev requires ALLOW_DEV_JWT_AUTH=true."
                            .to_string(),
                    );
                }

                Ok(Self {
                    mode: UserDirectoryMode::JwtClaimsDevelopment,
                })
            }
            _ => {
                Err("AUTH_USER_DIRECTORY must be either 'static' or 'jwt_claims_dev'.".to_string())
            }
        }
    }

    fn resolve_user(&self, claims: &Claims) -> Result<AuthUser, AuthError> {
        match &self.mode {
            UserDirectoryMode::Static(users) => {
                let Some(record) = users.get(&claims.sub) else {
                    return Err(AuthError::UnknownUser);
                };

                if !record.active {
                    return Err(AuthError::DisabledUser);
                }

                Ok(AuthUser {
                    id: claims.sub.clone(),
                    plan: record.plan,
                })
            }
            UserDirectoryMode::JwtClaimsDevelopment => Ok(AuthUser {
                id: claims.sub.clone(),
                plan: claims.plan.unwrap_or(UserPlan::Free),
            }),
        }
    }
}

pub fn bearer_token_from_header(value: Option<&str>) -> Result<&str, AuthError> {
    value
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(AuthError::MissingBearerToken)
}

pub fn authenticate_bearer_token(
    token: &str,
    jwt_config: &JwtConfig,
    user_directory: &UserDirectory,
) -> Result<AuthUser, AuthError> {
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_config.secret.as_bytes()),
        &jwt_validation(jwt_config),
    )
    .map_err(|_| AuthError::InvalidToken)?
    .claims;

    user_directory.resolve_user(&claims)
}

pub fn required_secret_env(key: &str) -> Result<String, String> {
    let value = required_env(key)?;
    validate_jwt_secret(key, &value)?;
    Ok(value)
}

pub fn validate_jwt_secret(key: &str, value: &str) -> Result<(), String> {
    let normalized = value.trim().to_ascii_lowercase();
    let weak_placeholders = [
        "replace_with_a_long_random_secret",
        "change_me",
        "changeme",
        "secret",
        "password",
    ];

    if value.len() < MIN_JWT_SECRET_BYTES {
        return Err(format!(
            "{key} must be at least {MIN_JWT_SECRET_BYTES} bytes long."
        ));
    }

    if weak_placeholders.contains(&normalized.as_str()) || normalized.starts_with("replace_") {
        return Err(format!(
            "{key} is still set to an unsafe placeholder value."
        ));
    }

    Ok(())
}

fn jwt_validation(config: &JwtConfig) -> Validation {
    let mut validation = Validation::new(Algorithm::HS256);

    if let Some(issuer) = &config.issuer {
        validation.set_issuer(&[issuer]);
        validation.set_required_spec_claims(&["exp", "iss"]);
    }

    if let Some(audience) = &config.audience {
        validation.set_audience(&[audience]);
        validation.set_required_spec_claims(if config.issuer.is_some() {
            &["exp", "iss", "aud"]
        } else {
            &["exp", "aud"]
        });
    }

    validation
}

fn parse_static_users(value: &str) -> Result<HashMap<String, UserRecord>, String> {
    let mut users = HashMap::new();

    for entry in value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let parts = entry.split(':').map(str::trim).collect::<Vec<_>>();

        if !(2..=3).contains(&parts.len()) {
            return Err(
                "AUTH_STATIC_USERS entries must use user_id:plan or user_id:plan:active."
                    .to_string(),
            );
        }

        let user_id = parts[0];
        if user_id.is_empty() {
            return Err("AUTH_STATIC_USERS contains an empty user id.".to_string());
        }

        let plan = parse_user_plan(parts[1])?;
        let active = parts
            .get(2)
            .map(|value| parse_active_flag(value))
            .transpose()?
            .unwrap_or(true);

        users.insert(user_id.to_string(), UserRecord { plan, active });
    }

    if users.is_empty() {
        return Err(
            "AUTH_STATIC_USERS must contain at least one active or disabled user.".to_string(),
        );
    }

    Ok(users)
}

fn parse_user_plan(value: &str) -> Result<UserPlan, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "free" => Ok(UserPlan::Free),
        "pro" => Ok(UserPlan::Pro),
        _ => Err("user plan must be either 'free' or 'pro'.".to_string()),
    }
}

fn parse_active_flag(value: &str) -> Result<bool, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "active" | "true" | "enabled" => Ok(true),
        "disabled" | "false" => Ok(false),
        _ => Err("user active flag must be active, enabled, true, disabled, or false.".to_string()),
    }
}

fn required_env(key: &str) -> Result<String, String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing required environment variable: {key}"))
}

fn read_env_or_default(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn env_flag_enabled(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestClaims {
        sub: String,
        plan: String,
        exp: usize,
    }

    fn test_jwt_config() -> JwtConfig {
        JwtConfig {
            secret: "test-secret-with-at-least-32-characters".to_string(),
            issuer: None,
            audience: None,
        }
    }

    fn test_token(user_id: &str, plan: &str) -> String {
        let claims = TestClaims {
            sub: user_id.to_string(),
            plan: plan.to_string(),
            exp: 4_102_444_800,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(test_jwt_config().secret.as_bytes()),
        )
        .expect("test token should encode")
    }

    #[test]
    fn static_users_parse_supported_entries() {
        let users =
            parse_static_users("user-1:free, user-2:pro:disabled").expect("valid static users");

        assert!(users.get("user-1").expect("user exists").active);
        assert_eq!(
            users.get("user-1").expect("user exists").plan,
            UserPlan::Free
        );
        assert!(!users.get("user-2").expect("user exists").active);
        assert_eq!(
            users.get("user-2").expect("user exists").plan,
            UserPlan::Pro
        );
    }

    #[test]
    fn static_users_reject_invalid_plan() {
        assert!(parse_static_users("user-1:enterprise").is_err());
    }

    #[test]
    fn bearer_token_requires_bearer_prefix() {
        assert_eq!(
            bearer_token_from_header(Some("Bearer token-value")).expect("valid token"),
            "token-value"
        );
        assert!(matches!(
            bearer_token_from_header(Some("Basic token-value")),
            Err(AuthError::MissingBearerToken)
        ));
    }

    #[test]
    fn static_directory_plan_overrides_jwt_claim_plan() {
        let directory =
            UserDirectory::from_static_users("user-1:free").expect("valid static users");
        let token = test_token("user-1", "pro");
        let user = authenticate_bearer_token(&token, &test_jwt_config(), &directory)
            .expect("known user should authenticate");

        assert_eq!(user.plan, UserPlan::Free);
    }

    #[test]
    fn static_directory_rejects_unknown_users() {
        let directory =
            UserDirectory::from_static_users("user-1:free").expect("valid static users");
        let token = test_token("unknown-user", "pro");
        let error = authenticate_bearer_token(&token, &test_jwt_config(), &directory)
            .expect_err("unknown user should fail");

        assert!(matches!(error, AuthError::UnknownUser));
    }

    #[test]
    fn rejects_placeholder_jwt_secret() {
        assert!(validate_jwt_secret(
            "JWT_SECRET",
            "replace_with_at_least_32_random_bytes_before_running"
        )
        .is_err());
    }
}
