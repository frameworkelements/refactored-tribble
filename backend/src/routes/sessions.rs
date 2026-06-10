use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{SessionEnrollee, SessionView};
use crate::state::AppState;
use crate::validation::{optional_str, validate_range};

const MAX_LOCATION: usize = 200;
const MAX_INSTRUCTOR: usize = 120;

/// Columns shared by every session listing query.
const SESSION_SELECT: &str = "\
    SELECT s.id, s.training_id, t.title AS training_title, s.starts_at, s.ends_at, \
           s.location, s.instructor, s.capacity, \
           (SELECT count(*) FROM session_enrollments e \
              WHERE e.session_id = s.id AND e.status = 'enrolled') AS enrolled_count, \
           EXISTS (SELECT 1 FROM session_enrollments e \
              WHERE e.session_id = s.id AND e.user_id = $1 AND e.status = 'enrolled') AS enrolled \
    FROM training_sessions s \
    JOIN trainings t ON t.id = s.training_id";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionInput {
    pub training_id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    #[serde(default)]
    pub location: Option<String>,
    #[serde(default)]
    pub instructor: Option<String>,
    #[serde(default)]
    pub capacity: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub training_id: Option<Uuid>,
    /// When true (the default), only sessions that have not yet finished.
    pub upcoming: Option<bool>,
}

fn validate(body: &SessionInput) -> AppResult<(String, Option<String>)> {
    if body.ends_at <= body.starts_at {
        return Err(AppError::bad_request("ends_at must be after starts_at"));
    }
    let location = optional_str("location", &body.location, MAX_LOCATION)?.unwrap_or_default();
    let instructor = optional_str("instructor", &body.instructor, MAX_INSTRUCTOR)?;
    if let Some(cap) = body.capacity {
        validate_range("capacity", cap, 1, 100_000)?;
    }
    Ok((location, instructor))
}

/// GET /api/sessions
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<SessionView>>> {
    let upcoming = q.upcoming.unwrap_or(true);
    let sql = format!(
        "{SESSION_SELECT} WHERE ($2::uuid IS NULL OR s.training_id = $2) \
         AND ($3 = false OR s.ends_at >= now()) ORDER BY s.starts_at ASC"
    );
    let rows = sqlx::query_as::<_, SessionView>(&sql)
        .bind(user.id)
        .bind(q.training_id)
        .bind(upcoming)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows))
}

/// GET /api/me/schedule — the current user's upcoming enrolled sessions.
pub async fn my_schedule(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<SessionView>>> {
    let sql = format!(
        "{SESSION_SELECT} WHERE s.ends_at >= now() AND EXISTS (\
            SELECT 1 FROM session_enrollments e \
            WHERE e.session_id = s.id AND e.user_id = $1 AND e.status = 'enrolled') \
         ORDER BY s.starts_at ASC"
    );
    let rows = sqlx::query_as::<_, SessionView>(&sql)
        .bind(user.id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows))
}

/// GET /api/sessions/:id
pub async fn get_one(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<SessionView>> {
    let sql = format!("{SESSION_SELECT} WHERE s.id = $2");
    let row = sqlx::query_as::<_, SessionView>(&sql)
        .bind(user.id)
        .bind(id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// POST /api/sessions — admin/manager only.
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<SessionInput>,
) -> AppResult<(StatusCode, Json<SessionView>)> {
    user.require_manager()?;
    let (location, instructor) = validate(&body)?;

    // Ensure the training exists for a clean 400 rather than an FK error.
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM trainings WHERE id = $1")
        .bind(body.training_id)
        .fetch_optional(&state.db)
        .await?;
    if exists.is_none() {
        return Err(AppError::bad_request("training does not exist"));
    }

    let (new_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO training_sessions \
            (training_id, starts_at, ends_at, location, instructor, capacity, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(body.training_id)
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(&location)
    .bind(&instructor)
    .bind(body.capacity)
    .bind(user.id)
    .fetch_one(&state.db)
    .await?;

    let sql = format!("{SESSION_SELECT} WHERE s.id = $2");
    let view = sqlx::query_as::<_, SessionView>(&sql)
        .bind(user.id)
        .bind(new_id)
        .fetch_one(&state.db)
        .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

/// PUT /api/sessions/:id — admin/manager only.
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SessionInput>,
) -> AppResult<Json<SessionView>> {
    user.require_manager()?;
    let (location, instructor) = validate(&body)?;

    let updated = sqlx::query(
        "UPDATE training_sessions \
         SET training_id = $1, starts_at = $2, ends_at = $3, location = $4, \
             instructor = $5, capacity = $6 \
         WHERE id = $7",
    )
    .bind(body.training_id)
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(&location)
    .bind(&instructor)
    .bind(body.capacity)
    .bind(id)
    .execute(&state.db)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    let sql = format!("{SESSION_SELECT} WHERE s.id = $2");
    let view = sqlx::query_as::<_, SessionView>(&sql)
        .bind(user.id)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(view))
}

/// DELETE /api/sessions/:id — admin/manager only.
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    user.require_manager()?;
    let result = sqlx::query("DELETE FROM training_sessions WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/sessions/:id/enrollments — admin/manager only.
pub async fn enrollments(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<SessionEnrollee>>> {
    user.require_manager()?;
    let rows = sqlx::query_as::<_, SessionEnrollee>(
        "SELECT e.user_id, u.email, e.status, e.enrolled_at \
         FROM session_enrollments e JOIN users u ON u.id = e.user_id \
         WHERE e.session_id = $1 ORDER BY e.enrolled_at ASC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// POST /api/sessions/:id/enroll — enroll the current user.
pub async fn enroll(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    // Pull the data needed to validate the enrollment in one round-trip.
    let row = sqlx::query_as::<_, (DateTime<Utc>, Option<i32>, i64, Option<String>)>(
        "SELECT s.starts_at, s.capacity, \
                (SELECT count(*) FROM session_enrollments e \
                   WHERE e.session_id = s.id AND e.status = 'enrolled') AS enrolled_count, \
                (SELECT e.status FROM session_enrollments e \
                   WHERE e.session_id = s.id AND e.user_id = $2) AS my_status \
         FROM training_sessions s WHERE s.id = $1",
    )
    .bind(id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (starts_at, capacity, enrolled_count, my_status) = row;

    if my_status.as_deref() == Some("enrolled") {
        return Ok(StatusCode::NO_CONTENT); // already enrolled — idempotent
    }
    if starts_at <= Utc::now() {
        return Err(AppError::bad_request("this session has already started"));
    }
    if let Some(cap) = capacity {
        if enrolled_count >= cap as i64 {
            return Err(AppError::conflict("this session is full"));
        }
    }

    sqlx::query(
        "INSERT INTO session_enrollments (session_id, user_id, status) \
         VALUES ($1, $2, 'enrolled') \
         ON CONFLICT (session_id, user_id) \
         DO UPDATE SET status = 'enrolled', enrolled_at = now()",
    )
    .bind(id)
    .bind(user.id)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/sessions/:id/cancel — cancel the current user's enrollment.
pub async fn cancel(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let result = sqlx::query(
        "UPDATE session_enrollments SET status = 'cancelled' \
         WHERE session_id = $1 AND user_id = $2 AND status = 'enrolled'",
    )
    .bind(id)
    .bind(user.id)
    .execute(&state.db)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::bad_request("you are not enrolled in this session"));
    }
    Ok(StatusCode::NO_CONTENT)
}
