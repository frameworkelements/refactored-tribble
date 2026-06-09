use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{anyhow_lite::Error as InternalError, AppError, AppResult};
use crate::models::Role;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "session";

/// The authenticated principal, attached to the request by `auth_middleware`
/// and extracted by handlers via `Extension<AuthUser>` or directly.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    #[allow(dead_code)]
    pub email: String,
    pub role: Role,
}

// ---------------------------------------------------------------------------
// Password hashing (Argon2id)
// ---------------------------------------------------------------------------

fn argon2() -> Argon2<'static> {
    // Argon2id with sensible interactive parameters.
    let params = Params::new(19_456, 2, 1, None).expect("valid argon2 params");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(InternalError::msg(format!("hash error: {e}"))))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    match PasswordHash::new(stored_hash) {
        Ok(parsed) => argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Session tokens
// ---------------------------------------------------------------------------

/// Generate a 256-bit random session token, returned as hex. The raw token is
/// handed to the client; only its SHA-256 hash is persisted server-side.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

/// Persist a new session and return the raw token.
pub async fn create_session(
    state: &AppState,
    user_id: Uuid,
) -> AppResult<(String, DateTime<Utc>)> {
    let token = generate_token();
    let token_hash = hash_token(&token);
    let expires_at = Utc::now()
        + chrono::Duration::from_std(state.config.session_ttl)
            .map_err(|e| AppError::Internal(InternalError::msg(e.to_string())))?;

    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(&token_hash)
        .bind(user_id)
        .bind(expires_at)
        .execute(&state.db)
        .await?;

    Ok((token, expires_at))
}

pub async fn destroy_session(state: &AppState, token: &str) -> AppResult<()> {
    let token_hash = hash_token(token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(&token_hash)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Look up the user for a raw token, enforcing expiry. Returns None when the
/// token is unknown or expired.
async fn user_for_token(state: &AppState, token: &str) -> AppResult<Option<AuthUser>> {
    let token_hash = hash_token(token);
    let row = sqlx::query_as::<_, (Uuid, String, Role)>(
        r#"
        SELECT u.id, u.email, u.role
        FROM sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token_hash = $1 AND s.expires_at > now()
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(&state.db)
    .await?;

    Ok(row.map(|(id, email, role)| AuthUser { id, email, role }))
}

// ---------------------------------------------------------------------------
// Middleware + extractor
// ---------------------------------------------------------------------------

/// Authentication middleware applied to every protected route. Reads the
/// session cookie, validates it, and injects `AuthUser` into request
/// extensions. Rejects with 401 when no valid session is present.
pub async fn auth_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = jar
        .get(SESSION_COOKIE)
        .map(|c| c.value().to_string())
        .ok_or(AppError::Unauthorized)?;

    let user = user_for_token(&state, &token)
        .await?
        .ok_or(AppError::Unauthorized)?;

    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}

/// Allows handlers to take `AuthUser` directly as an argument. Relies on
/// `auth_middleware` having populated the extension.
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or(AppError::Unauthorized)
    }
}

impl AuthUser {
    pub fn require_manager(&self) -> AppResult<()> {
        if self.role.can_manage() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
    pub fn require_admin(&self) -> AppResult<()> {
        if self.role.is_admin() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

// ---------------------------------------------------------------------------
// Seed admin bootstrap
// ---------------------------------------------------------------------------

/// Create the seed admin on first run if it does not already exist. Credentials
/// come exclusively from the environment, so nothing sensitive lives in source.
pub async fn ensure_seed_admin(state: &AppState) -> AppResult<()> {
    let (email, password) = match (
        state.config.seed_admin_email.as_ref(),
        state.config.seed_admin_password.as_ref(),
    ) {
        (Some(e), Some(p)) => (e, p),
        _ => {
            tracing::info!("SEED_ADMIN_EMAIL/PASSWORD not set; skipping seed admin bootstrap");
            return Ok(());
        }
    };

    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(&state.db)
        .await?;

    if exists.is_some() {
        tracing::info!(%email, "seed admin already present");
        return Ok(());
    }

    let hash = hash_password(password)?;
    sqlx::query("INSERT INTO users (email, password_hash, role) VALUES ($1, $2, 'admin')")
        .bind(email)
        .bind(&hash)
        .execute(&state.db)
        .await?;

    tracing::info!(%email, "seed admin created");
    Ok(())
}
