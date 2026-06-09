use std::time::Duration;

/// Runtime configuration, sourced entirely from environment variables.
/// No secret has a default — the process refuses to start without them.
#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    /// Key used to derive the HMAC of session tokens stored server-side.
    pub session_secret: String,
    pub session_ttl: Duration,
    pub cookie_secure: bool,
    pub seed_admin_email: Option<String>,
    pub seed_admin_password: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url = required("DATABASE_URL")?;
        let session_secret = required("SESSION_SECRET")?;
        if session_secret.len() < 16 {
            return Err("SESSION_SECRET must be at least 16 characters".into());
        }

        let bind_addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());

        let session_ttl_hours: u64 = std::env::var("SESSION_TTL_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(12);

        // Secure cookies are the default. Operators running plain HTTP locally
        // can opt out with COOKIE_SECURE=false.
        let cookie_secure = std::env::var("COOKIE_SECURE")
            .map(|v| v != "false" && v != "0")
            .unwrap_or(true);

        Ok(Self {
            database_url,
            bind_addr,
            session_secret,
            session_ttl: Duration::from_secs(session_ttl_hours * 3600),
            cookie_secure,
            seed_admin_email: non_empty("SEED_ADMIN_EMAIL"),
            seed_admin_password: non_empty("SEED_ADMIN_PASSWORD"),
        })
    }
}

fn required(key: &str) -> Result<String, String> {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("missing required environment variable: {key}"))
}

fn non_empty(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.is_empty())
}
