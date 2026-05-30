use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::Serialize;

const MIN_JWT_SECRET_BYTES: usize = 32;
const MAX_TOKEN_DAYS: i64 = 365;

#[derive(Serialize)]
struct Claims {
    sub: String,
    plan: String,
    exp: usize,
    iat: usize,
    jti: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    iss: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    dotenvy::dotenv().ok();

    let jwt_secret = required_secret_env("JWT_SECRET")?;
    let user_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dev-user".to_string());
    let plan = validate_plan(
        &std::env::args()
            .nth(2)
            .unwrap_or_else(|| "free".to_string()),
    )?;
    let days = validate_days(std::env::args().nth(3).unwrap_or_else(|| "30".to_string()))?;

    let now = Utc::now();
    let expires_at = now + Duration::days(days);

    let claims = Claims {
        sub: clean_user_id(user_id)?,
        plan,
        exp: expires_at.timestamp() as usize,
        iat: now.timestamp() as usize,
        jti: format!("{}-{}", now.timestamp_millis(), std::process::id()),
        iss: read_optional_env("JWT_ISSUER"),
        aud: read_optional_env("JWT_AUDIENCE"),
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|error| format!("failed to mint JWT: {error}"))?;

    println!("{token}");

    Ok(())
}

fn required_secret_env(key: &str) -> Result<String, String> {
    let value = std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{key} must be set in the environment or backend/.env"))?;

    validate_jwt_secret(key, &value)?;
    Ok(value)
}

fn validate_jwt_secret(key: &str, value: &str) -> Result<(), String> {
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

fn clean_user_id(user_id: String) -> Result<String, String> {
    let user_id = user_id.trim().to_string();

    if user_id.is_empty() {
        Err("user_id cannot be empty.".to_string())
    } else {
        Ok(user_id)
    }
}

fn validate_plan(plan: &str) -> Result<String, String> {
    match plan.trim().to_ascii_lowercase().as_str() {
        "free" => Ok("free".to_string()),
        "pro" => Ok("pro".to_string()),
        _ => Err("plan must be either 'free' or 'pro'.".to_string()),
    }
}

fn validate_days(value: String) -> Result<i64, String> {
    let days = value
        .trim()
        .parse::<i64>()
        .map_err(|_| "days must be a whole number.".to_string())?;

    if !(1..=MAX_TOKEN_DAYS).contains(&days) {
        return Err(format!("days must be between 1 and {MAX_TOKEN_DAYS}."));
    }

    Ok(days)
}

fn read_optional_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_plan() {
        assert!(validate_plan("enterprise").is_err());
    }

    #[test]
    fn accepts_supported_plans_case_insensitively() {
        assert_eq!(validate_plan("FREE").expect("valid plan"), "free");
        assert_eq!(validate_plan("pro").expect("valid plan"), "pro");
    }

    #[test]
    fn rejects_out_of_range_days() {
        assert!(validate_days("0".to_string()).is_err());
        assert!(validate_days("366".to_string()).is_err());
        assert!(validate_days("-1".to_string()).is_err());
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
