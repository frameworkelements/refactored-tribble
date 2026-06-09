//! Single sign-on via OpenID Connect (OAuth 2.0 Authorization Code flow with
//! PKCE). Works with any OIDC-compliant identity provider (Google, Microsoft
//! Entra ID, Okta, Auth0, Keycloak, …) discovered from its issuer URL.
//!
//! SSO is just another way to obtain the application's existing server-side
//! session: after the ID token is verified, the user is provisioned/linked and
//! the same session cookie used by password login is issued.

use axum::extract::{Query, State};
use axum::response::Redirect;
use axum::Json;
use axum_extra::extract::cookie::CookieJar;
use chrono::{Duration, Utc};
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    reqwest, AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, Scope,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::create_session;
use crate::config::OidcSettings;
use crate::error::{anyhow_lite::Error as InternalError, AppError, AppResult};
use crate::models::Role;
use crate::routes::auth::build_session_cookie;
use crate::state::AppState;

/// Discovered OIDC provider state, built once at startup and shared.
pub struct OidcState {
    pub metadata: CoreProviderMetadata,
    pub http: reqwest::Client,
    pub settings: OidcSettings,
}

impl OidcState {
    /// Build the HTTP client and discover provider metadata from the issuer.
    pub async fn init(settings: OidcSettings) -> Result<Self, String> {
        // Disabling redirects on the client mitigates SSRF via the token/JWKS
        // endpoints (per the openidconnect crate's guidance).
        let http = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| format!("failed to build OIDC http client: {e}"))?;

        let issuer = IssuerUrl::new(settings.issuer_url.clone())
            .map_err(|e| format!("invalid OIDC_ISSUER_URL: {e}"))?;

        let metadata = CoreProviderMetadata::discover_async(issuer, &http)
            .await
            .map_err(|e| format!("OIDC discovery failed: {e}"))?;

        Ok(Self {
            metadata,
            http,
            settings,
        })
    }
}

fn internal<E: std::fmt::Display>(e: E) -> AppError {
    AppError::Internal(InternalError::msg(e.to_string()))
}

/// GET /api/auth/sso/status — public; tells the frontend whether to show the
/// SSO button.
pub async fn status(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "enabled": state.oidc.is_some() }))
}

/// GET /api/auth/sso/login — begin the flow: redirect the browser to the IdP.
pub async fn login(State(state): State<AppState>) -> AppResult<Redirect> {
    let oidc = state.oidc.as_ref().ok_or(AppError::NotFound)?;

    let client = CoreClient::from_provider_metadata(
        oidc.metadata.clone(),
        ClientId::new(oidc.settings.client_id.clone()),
        Some(ClientSecret::new(oidc.settings.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(oidc.settings.redirect_url.clone()).map_err(internal)?);

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let (auth_url, csrf_token, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("openid".to_string()))
        .add_scope(Scope::new("email".to_string()))
        .add_scope(Scope::new("profile".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Persist the per-attempt secrets server-side, keyed by the CSRF state,
    // for one-time use within a short window.
    let expires_at = Utc::now() + Duration::minutes(10);
    sqlx::query(
        "INSERT INTO oidc_auth_requests (state, pkce_verifier, nonce, expires_at) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(csrf_token.secret())
    .bind(pkce_verifier.secret())
    .bind(nonce.secret())
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    Ok(Redirect::to(auth_url.as_str()))
}

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// GET /api/auth/sso/callback — IdP redirects back here with an auth code.
pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(params): Query<CallbackParams>,
) -> AppResult<(CookieJar, Redirect)> {
    let oidc = state.oidc.as_ref().ok_or(AppError::NotFound)?;

    // The user denied consent or the IdP returned an error.
    if let Some(err) = params.error {
        tracing::warn!("SSO provider returned error: {err}");
        return Ok((jar, Redirect::to("/login?sso_error=1")));
    }

    let code = params
        .code
        .ok_or_else(|| AppError::bad_request("missing authorization code"))?;
    let csrf_state = params
        .state
        .ok_or_else(|| AppError::bad_request("missing state"))?;

    // Atomically consume the stored auth request (one-time use, CSRF defence).
    let row = sqlx::query_as::<_, (String, String, chrono::DateTime<Utc>)>(
        "DELETE FROM oidc_auth_requests WHERE state = $1 \
         RETURNING pkce_verifier, nonce, expires_at",
    )
    .bind(&csrf_state)
    .fetch_optional(&state.db)
    .await?;

    let (pkce_verifier, nonce, expires_at) =
        row.ok_or_else(|| AppError::bad_request("invalid or unknown SSO state"))?;
    if expires_at < Utc::now() {
        return Err(AppError::bad_request("SSO login attempt expired"));
    }

    let client = CoreClient::from_provider_metadata(
        oidc.metadata.clone(),
        ClientId::new(oidc.settings.client_id.clone()),
        Some(ClientSecret::new(oidc.settings.client_secret.clone())),
    )
    .set_redirect_uri(RedirectUrl::new(oidc.settings.redirect_url.clone()).map_err(internal)?);

    // Exchange the code for tokens, binding the PKCE verifier.
    let token_response = client
        .exchange_code(AuthorizationCode::new(code))
        .map_err(internal)?
        .set_pkce_verifier(PkceCodeVerifier::new(pkce_verifier))
        .request_async(&oidc.http)
        .await
        .map_err(|e| {
            tracing::warn!("SSO token exchange failed: {e}");
            AppError::Unauthorized
        })?;

    // Verify the ID token signature (via JWKS), issuer, audience, expiry and
    // the nonce we issued.
    let verifier = client.id_token_verifier();
    let id_token = token_response
        .extra_fields()
        .id_token()
        .ok_or_else(|| AppError::bad_request("identity provider did not return an ID token"))?;
    let claims = id_token
        .claims(&verifier, &Nonce::new(nonce))
        .map_err(|e| {
            tracing::warn!("ID token verification failed: {e}");
            AppError::Unauthorized
        })?;

    let email = claims
        .email()
        .map(|e| e.as_str().trim().to_lowercase())
        .filter(|e| !e.is_empty())
        .ok_or_else(|| AppError::bad_request("identity provider did not return an email"))?;

    // Only accept verified emails — otherwise account linking could be spoofed.
    if claims.email_verified() != Some(true) {
        return Err(AppError::bad_request("email address is not verified by the provider"));
    }

    if let Some(domain) = &oidc.settings.allowed_email_domain {
        if !email.ends_with(&format!("@{domain}")) {
            return Err(AppError::Forbidden);
        }
    }

    let subject = claims.subject().as_str();
    let user_id =
        provision_user(&state, &oidc.settings.issuer_url, subject, &email, oidc.settings.default_role)
            .await?;

    let (token, expires) = create_session(&state, user_id).await?;
    let max_age = (expires - Utc::now()).num_seconds().max(0);
    let cookie = build_session_cookie(token, state.config.cookie_secure, max_age);

    Ok((
        jar.add(cookie),
        Redirect::to(&oidc.settings.post_login_redirect),
    ))
}

/// Find an existing user for these SSO claims, or provision one just-in-time.
/// Matching precedence: (issuer, subject) → email (link) → create.
async fn provision_user(
    state: &AppState,
    issuer: &str,
    subject: &str,
    email: &str,
    default_role: Role,
) -> AppResult<Uuid> {
    // 1. Stable identity: issuer + subject.
    if let Some((id,)) = sqlx::query_as::<_, (Uuid,)>(
        "SELECT id FROM users WHERE oidc_issuer = $1 AND oidc_subject = $2",
    )
    .bind(issuer)
    .bind(subject)
    .fetch_optional(&state.db)
    .await?
    {
        return Ok(id);
    }

    // 2. Existing local/email account: link it to this SSO identity.
    if let Some((id,)) = sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(&state.db)
        .await?
    {
        sqlx::query("UPDATE users SET oidc_issuer = $1, oidc_subject = $2 WHERE id = $3")
            .bind(issuer)
            .bind(subject)
            .bind(id)
            .execute(&state.db)
            .await?;
        return Ok(id);
    }

    // 3. Provision a new account (no password hash — SSO only).
    let created = sqlx::query_as::<_, (Uuid,)>(
        "INSERT INTO users (email, role, oidc_issuer, oidc_subject) \
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(email)
    .bind(default_role)
    .bind(issuer)
    .bind(subject)
    .fetch_one(&state.db)
    .await;

    match created {
        Ok((id,)) => Ok(id),
        // Lost a race with a concurrent first login — re-read the winner.
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
            let (id,) = sqlx::query_as::<_, (Uuid,)>(
                "SELECT id FROM users WHERE oidc_issuer = $1 AND oidc_subject = $2",
            )
            .bind(issuer)
            .bind(subject)
            .fetch_one(&state.db)
            .await?;
            Ok(id)
        }
        Err(e) => Err(e.into()),
    }
}
