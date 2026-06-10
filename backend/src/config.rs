use std::time::Duration;

use crate::models::Role;

/// Optional OpenID Connect (SSO) settings. Present only when all four required
/// variables are configured.
#[derive(Clone, Debug)]
pub struct OidcSettings {
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    /// Role granted to users provisioned just-in-time on first SSO login.
    pub default_role: Role,
    /// When set, only emails in this domain may sign in via SSO.
    pub allowed_email_domain: Option<String>,
    /// Where the browser is sent after a successful SSO login.
    pub post_login_redirect: String,
}

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
    pub oidc: Option<OidcSettings>,
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
            oidc: oidc_from_env()?,
        })
    }
}

/// Build OIDC settings if (and only if) all required variables are present.
fn oidc_from_env() -> Result<Option<OidcSettings>, String> {
    let issuer_url = non_empty("OIDC_ISSUER_URL");
    let client_id = non_empty("OIDC_CLIENT_ID");
    let client_secret = non_empty("OIDC_CLIENT_SECRET");
    let redirect_url = non_empty("OIDC_REDIRECT_URL");

    match (issuer_url, client_id, client_secret, redirect_url) {
        (Some(issuer_url), Some(client_id), Some(client_secret), Some(redirect_url)) => {
            let default_role = match non_empty("OIDC_DEFAULT_ROLE").as_deref() {
                None | Some("learner") => Role::Learner,
                Some("manager") => Role::Manager,
                Some("admin") => Role::Admin,
                Some(other) => {
                    return Err(format!("invalid OIDC_DEFAULT_ROLE: {other}"));
                }
            };
            Ok(Some(OidcSettings {
                issuer_url,
                client_id,
                client_secret,
                redirect_url,
                default_role,
                allowed_email_domain: non_empty("OIDC_ALLOWED_EMAIL_DOMAIN")
                    .map(|d| d.trim().trim_start_matches('@').to_lowercase()),
                post_login_redirect: non_empty("POST_LOGIN_REDIRECT")
                    .unwrap_or_else(|| "/".to_string()),
            }))
        }
        (None, None, None, None) => Ok(None),
        _ => Err(
            "incomplete OIDC configuration: set all of OIDC_ISSUER_URL, OIDC_CLIENT_ID, \
             OIDC_CLIENT_SECRET, OIDC_REDIRECT_URL or none of them"
                .into(),
        ),
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
