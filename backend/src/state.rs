use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::ratelimit::LoginRateLimiter;
use crate::routes::oidc::OidcState;

/// Shared application state, cloned cheaply into every handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub login_limiter: Arc<LoginRateLimiter>,
    /// Present only when SSO is configured and discovery succeeded.
    pub oidc: Option<Arc<OidcState>>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config, oidc: Option<Arc<OidcState>>) -> Self {
        Self {
            db,
            config: Arc::new(config),
            login_limiter: Arc::new(LoginRateLimiter::new()),
            oidc,
        }
    }
}
