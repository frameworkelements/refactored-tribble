use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::Training;
use crate::state::AppState;
use crate::validation::{optional_str, required_str, validate_range};

const MAX_TITLE: usize = 200;
const MAX_DESCRIPTION: usize = 5000;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingInput {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub duration_minutes: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteInput {
    #[serde(default)]
    pub score: Option<i32>,
}

/// GET /api/trainings
pub async fn list(State(state): State<AppState>, _user: AuthUser) -> AppResult<Json<Vec<Training>>> {
    let rows = sqlx::query_as::<_, Training>(
        "SELECT id, title, description, duration_minutes, created_by, created_at, updated_at \
         FROM trainings ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// POST /api/trainings — admin/manager only.
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<TrainingInput>,
) -> AppResult<(StatusCode, Json<Training>)> {
    user.require_manager()?;
    let title = required_str("title", &body.title, MAX_TITLE)?;
    let description = optional_str("description", &body.description, MAX_DESCRIPTION)?
        .unwrap_or_default();
    validate_range("duration_minutes", body.duration_minutes, 0, 100_000)?;

    let row = sqlx::query_as::<_, Training>(
        "INSERT INTO trainings (title, description, duration_minutes, created_by) \
         VALUES ($1, $2, $3, $4) \
         RETURNING id, title, description, duration_minutes, created_by, created_at, updated_at",
    )
    .bind(&title)
    .bind(&description)
    .bind(body.duration_minutes)
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /api/trainings/:id
pub async fn get_one(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Training>> {
    let row = sqlx::query_as::<_, Training>(
        "SELECT id, title, description, duration_minutes, created_by, created_at, updated_at \
         FROM trainings WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// PUT /api/trainings/:id — admin/manager only.
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<TrainingInput>,
) -> AppResult<Json<Training>> {
    user.require_manager()?;
    let title = required_str("title", &body.title, MAX_TITLE)?;
    let description = optional_str("description", &body.description, MAX_DESCRIPTION)?
        .unwrap_or_default();
    validate_range("duration_minutes", body.duration_minutes, 0, 100_000)?;

    let row = sqlx::query_as::<_, Training>(
        "UPDATE trainings SET title = $1, description = $2, duration_minutes = $3 \
         WHERE id = $4 \
         RETURNING id, title, description, duration_minutes, created_by, created_at, updated_at",
    )
    .bind(&title)
    .bind(&description)
    .bind(body.duration_minutes)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// DELETE /api/trainings/:id — admin/manager only.
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    user.require_manager()?;
    let result = sqlx::query("DELETE FROM trainings WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/trainings/:id/complete — log a completion for the current user.
pub async fn complete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CompleteInput>,
) -> AppResult<StatusCode> {
    if let Some(score) = body.score {
        validate_range("score", score, 0, 100)?;
    }

    // Ensure the training exists for a clean 404 rather than an FK error.
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM trainings WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    sqlx::query(
        "INSERT INTO user_training_completions (user_id, training_id, completed_at, score) \
         VALUES ($1, $2, now(), $3) \
         ON CONFLICT (user_id, training_id) \
         DO UPDATE SET completed_at = now(), score = EXCLUDED.score",
    )
    .bind(user.id)
    .bind(id)
    .bind(body.score)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
