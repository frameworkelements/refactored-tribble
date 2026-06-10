use axum::extract::State;
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use serde::Deserialize;

use crate::auth::{
    create_session, destroy_session, hash_password, verify_password, AuthUser, SESSION_COOKIE,
};
use crate::error::{AppError, AppResult};
use crate::models::{Role, UserProfile};
use crate::state::AppState;
use crate::validation::{validate_email, validate_password};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// POST /api/auth/login — verify credentials and issue a session cookie.
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LoginRequest>,
) -> AppResult<(CookieJar, Json<UserProfile>)> {
    let email = validate_email(&body.email)?;
    validate_password(&body.password)?;

    // Throttle repeated failures per account to slow credential brute-forcing.
    if !state.login_limiter.check(&email) {
        return Err(AppError::TooManyRequests);
    }

    // Always run a hash verification to keep timing roughly constant whether or
    // not the account exists, mitigating user-enumeration via response timing.
    let user = sqlx::query_as::<_, (uuid::Uuid, String, Option<String>, Role)>(
        "SELECT id, email, password_hash, role FROM users WHERE email = $1",
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;

    let authenticated = match &user {
        // Account exists and has a password set.
        Some((_, _, Some(hash), _)) => verify_password(&body.password, hash),
        // Either no such account, or an SSO-only account with no password.
        // Run a dummy verification to keep timing roughly constant.
        _ => {
            let dummy = hash_password("invalid-placeholder-password")?;
            let _ = verify_password(&body.password, &dummy);
            false
        }
    };

    if !authenticated {
        state.login_limiter.record_failure(&email);
        return Err(AppError::Unauthorized);
    }
    state.login_limiter.reset(&email);

    let (id, email, _, role) = user.expect("authenticated implies user exists");
    let (token, expires_at) = create_session(&state, id).await?;

    let max_age = (expires_at - chrono::Utc::now()).num_seconds().max(0);
    let cookie = build_session_cookie(token, state.config.cookie_secure, max_age);

    let profile = UserProfile {
        id,
        email,
        role,
        created_at: chrono::Utc::now(),
    };

    Ok((jar.add(cookie), Json(profile)))
}

/// POST /api/auth/logout — invalidate the current session.
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<(CookieJar, Json<serde_json::Value>)> {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        destroy_session(&state, cookie.value()).await?;
    }
    let mut removal = Cookie::new(SESSION_COOKIE, "");
    removal.set_path("/");
    Ok((jar.remove(removal), Json(serde_json::json!({ "ok": true }))))
}

/// GET /api/me — current user's profile.
pub async fn me(State(state): State<AppState>, user: AuthUser) -> AppResult<Json<UserProfile>> {
    let profile = sqlx::query_as::<_, UserProfile>(
        "SELECT id, email, role, created_at FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(profile))
}

pub(crate) fn build_session_cookie(token: String, secure: bool, max_age_secs: i64) -> Cookie<'static> {
    let mut cookie = Cookie::new(SESSION_COOKIE, token);
    cookie.set_http_only(true);
    cookie.set_secure(secure);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(max_age_secs));
    cookie
}
