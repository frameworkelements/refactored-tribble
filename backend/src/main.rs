mod auth;
mod config;
mod error;
mod models;
mod ratelimit;
mod routes;
mod state;
mod validation;

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lms_backend=info,tower_http=info,sqlx=warn".into()),
        )
        .init();

    if let Err(e) = run().await {
        tracing::error!("fatal: {e}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let config = Config::from_env()?;

    let pool = connect_with_retry(&config.database_url).await?;

    // Optionally bring up SSO. Discovery failure is non-fatal: the app still
    // starts with password login, SSO simply stays disabled.
    let oidc = match config.oidc.clone() {
        Some(settings) => match routes::oidc::OidcState::init(settings).await {
            Ok(state) => {
                tracing::info!("OIDC SSO enabled");
                Some(std::sync::Arc::new(state))
            }
            Err(e) => {
                tracing::warn!("OIDC SSO disabled: {e}");
                None
            }
        },
        None => None,
    };

    let state = AppState::new(pool, config.clone(), oidc);

    // Bootstrap the seed admin (idempotent).
    auth::ensure_seed_admin(&state)
        .await
        .map_err(|_| "failed to bootstrap seed admin".to_string())?;

    // Periodically purge expired sessions (storage limitation / GDPR).
    spawn_session_cleanup(state.clone());

    let app = routes::router(state).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .map_err(|e| format!("failed to bind {}: {e}", config.bind_addr))?;

    tracing::info!("listening on {}", config.bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("server error: {e}"))?;

    Ok(())
}

/// Connect to Postgres, retrying for a while so the app can start alongside a
/// still-initializing database even though compose also gates on healthcheck.
async fn connect_with_retry(database_url: &str) -> Result<sqlx::PgPool, String> {
    let mut attempt = 0;
    loop {
        attempt += 1;
        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(e) if attempt < 10 => {
                tracing::warn!("db connection attempt {attempt} failed: {e}; retrying");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(format!("could not connect to database: {e}")),
        }
    }
}

/// Background task that deletes expired session rows so the session store does
/// not retain stale data indefinitely.
fn spawn_session_cleanup(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            match sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
                .execute(&state.db)
                .await
            {
                Ok(res) if res.rows_affected() > 0 => {
                    tracing::info!("purged {} expired session(s)", res.rows_affected());
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("session cleanup failed: {e}"),
            }
            // Also drop abandoned/expired in-flight SSO login attempts.
            if let Err(e) = sqlx::query("DELETE FROM oidc_auth_requests WHERE expires_at <= now()")
                .execute(&state.db)
                .await
            {
                tracing::warn!("oidc request cleanup failed: {e}");
            }
        }
    });
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
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}
