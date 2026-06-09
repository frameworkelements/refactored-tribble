use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;
use crate::ratelimit::LoginRateLimiter;

/// Shared application state, cloned cheaply into every handler.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<Config>,
    pub login_limiter: Arc<LoginRateLimiter>,
}

impl AppState {
    pub fn new(db: PgPool, config: Config) -> Self {
        Self {
            db,
            config: Arc::new(config),
            login_limiter: Arc::new(LoginRateLimiter::new()),
        }
    }
}
