pub mod auth;
pub mod certifications;
pub mod oidc;
pub mod reports;
pub mod trainings;
pub mod users;

use axum::routing::{get, post};
use axum::{middleware, Router};

use crate::auth::auth_middleware;
use crate::state::AppState;

/// Build the full API router. Public routes (`/health`, `/api/auth/login`)
/// are reachable without a session; everything else sits behind the auth
/// middleware layer.
pub fn router(state: AppState) -> Router {
    let public = Router::new()
        .route("/health", get(health))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/sso/status", get(oidc::status))
        .route("/api/auth/sso/login", get(oidc::login))
        .route("/api/auth/sso/callback", get(oidc::callback));

    let protected = Router::new()
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/me", get(auth::me))
        .route(
            "/api/trainings",
            get(trainings::list).post(trainings::create),
        )
        .route(
            "/api/trainings/{id}",
            get(trainings::get_one)
                .put(trainings::update)
                .delete(trainings::delete),
        )
        .route(
            "/api/trainings/{id}/complete",
            post(trainings::complete),
        )
        .route(
            "/api/certifications",
            get(certifications::list).post(certifications::create),
        )
        .route(
            "/api/certifications/{id}",
            get(certifications::get_one)
                .put(certifications::update)
                .delete(certifications::delete),
        )
        .route(
            "/api/certifications/{id}/assign",
            post(certifications::assign),
        )
        .route("/api/users", get(users::list).post(users::create))
        .route("/api/users/{id}", axum::routing::delete(users::delete))
        .route("/api/users/{id}/dashboard", get(users::dashboard))
        .route("/api/reports/compliance", get(reports::compliance))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}
