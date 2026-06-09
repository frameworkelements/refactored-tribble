use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::Certification;
use crate::state::AppState;
use crate::validation::{optional_str, required_str, validate_range};

const MAX_NAME: usize = 200;
const MAX_ISSUING_BODY: usize = 200;
const MAX_URL: usize = 2048;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertificationInput {
    pub name: String,
    pub issuing_body: String,
    pub validity_months: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignInput {
    pub user_id: Uuid,
    pub issued_date: NaiveDate,
    pub expiry_date: NaiveDate,
    #[serde(default)]
    pub document_url: Option<String>,
}

/// GET /api/certifications
pub async fn list(
    State(state): State<AppState>,
    _user: AuthUser,
) -> AppResult<Json<Vec<Certification>>> {
    let rows = sqlx::query_as::<_, Certification>(
        "SELECT id, name, issuing_body, validity_months, created_at, updated_at \
         FROM certifications ORDER BY name ASC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// POST /api/certifications — admin/manager only.
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<CertificationInput>,
) -> AppResult<(StatusCode, Json<Certification>)> {
    user.require_manager()?;
    let name = required_str("name", &body.name, MAX_NAME)?;
    let issuing_body = required_str("issuing_body", &body.issuing_body, MAX_ISSUING_BODY)?;
    validate_range("validity_months", body.validity_months, 1, 1200)?;

    let row = sqlx::query_as::<_, Certification>(
        "INSERT INTO certifications (name, issuing_body, validity_months) \
         VALUES ($1, $2, $3) \
         RETURNING id, name, issuing_body, validity_months, created_at, updated_at",
    )
    .bind(&name)
    .bind(&issuing_body)
    .bind(body.validity_months)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// GET /api/certifications/:id
pub async fn get_one(
    State(state): State<AppState>,
    _user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Certification>> {
    let row = sqlx::query_as::<_, Certification>(
        "SELECT id, name, issuing_body, validity_months, created_at, updated_at \
         FROM certifications WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// PUT /api/certifications/:id — admin/manager only.
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CertificationInput>,
) -> AppResult<Json<Certification>> {
    user.require_manager()?;
    let name = required_str("name", &body.name, MAX_NAME)?;
    let issuing_body = required_str("issuing_body", &body.issuing_body, MAX_ISSUING_BODY)?;
    validate_range("validity_months", body.validity_months, 1, 1200)?;

    let row = sqlx::query_as::<_, Certification>(
        "UPDATE certifications SET name = $1, issuing_body = $2, validity_months = $3 \
         WHERE id = $4 \
         RETURNING id, name, issuing_body, validity_months, created_at, updated_at",
    )
    .bind(&name)
    .bind(&issuing_body)
    .bind(body.validity_months)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

/// DELETE /api/certifications/:id — admin/manager only.
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    user.require_manager()?;
    let result = sqlx::query("DELETE FROM certifications WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/certifications/:id/assign — assign a cert record to a user.
pub async fn assign(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AssignInput>,
) -> AppResult<StatusCode> {
    user.require_manager()?;
    if body.expiry_date < body.issued_date {
        return Err(AppError::bad_request(
            "expiry_date must not be before issued_date",
        ));
    }
    let document_url = optional_str("document_url", &body.document_url, MAX_URL)?;

    // Verify both the certification and target user exist for clean 404s.
    let cert: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM certifications WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    if cert.is_none() {
        return Err(AppError::NotFound);
    }
    let target: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
        .bind(body.user_id)
        .fetch_optional(&state.db)
        .await?;
    if target.is_none() {
        return Err(AppError::bad_request("target user does not exist"));
    }

    sqlx::query(
        "INSERT INTO user_certifications \
            (user_id, certification_id, issued_date, expiry_date, document_url) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id, certification_id) DO UPDATE SET \
            issued_date = EXCLUDED.issued_date, \
            expiry_date = EXCLUDED.expiry_date, \
            document_url = EXCLUDED.document_url",
    )
    .bind(body.user_id)
    .bind(id)
    .bind(body.issued_date)
    .bind(body.expiry_date)
    .bind(&document_url)
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}
