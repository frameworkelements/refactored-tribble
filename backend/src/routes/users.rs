use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::{hash_password, AuthUser};
use crate::error::{AppError, AppResult};
use crate::models::{CertificationStatus, CompletionRecord, Dashboard, Role, UserProfile};
use crate::state::AppState;
use crate::validation::{validate_email, validate_password};

/// Number of days before expiry at which a certification is flagged.
pub const EXPIRY_WARNING_DAYS: i64 = 30;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUserInput {
    pub email: String,
    pub password: String,
    pub role: Role,
}

/// GET /api/users — admin/manager only (used by the admin panel).
pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<UserProfile>>> {
    user.require_manager()?;
    let rows = sqlx::query_as::<_, UserProfile>(
        "SELECT id, email, role, created_at FROM users ORDER BY email ASC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows))
}

/// POST /api/users — admin only. Creates a managed account.
pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<CreateUserInput>,
) -> AppResult<(StatusCode, Json<UserProfile>)> {
    user.require_admin()?;
    let email = validate_email(&body.email)?;
    validate_password(&body.password)?;
    let hash = hash_password(&body.password)?;

    let result = sqlx::query_as::<_, UserProfile>(
        "INSERT INTO users (email, password_hash, role) VALUES ($1, $2, $3) \
         RETURNING id, email, role, created_at",
    )
    .bind(&email)
    .bind(&hash)
    .bind(body.role)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(profile) => Ok((StatusCode::CREATED, Json(profile))),
        Err(sqlx::Error::Database(db)) if db.is_unique_violation() => {
            Err(AppError::conflict("a user with that email already exists"))
        }
        Err(e) => Err(e.into()),
    }
}

/// DELETE /api/users/:id — admin only. Right to erasure (GDPR Art. 17):
/// removes the account and cascades to the user's sessions, training
/// completions and certification records; authored trainings are retained but
/// their `created_by` link is nulled.
pub async fn delete(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    user.require_admin()?;

    // Guard against an admin locking everyone out by deleting their own
    // account or removing the final admin.
    if id == user.id {
        return Err(AppError::bad_request(
            "you cannot delete your own account",
        ));
    }
    let target_role: Option<(Role,)> = sqlx::query_as("SELECT role FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    let target_role = target_role.ok_or(AppError::NotFound)?.0;
    if target_role.is_admin() {
        let admin_count: (i64,) =
            sqlx::query_as("SELECT count(*) FROM users WHERE role = 'admin'")
                .fetch_one(&state.db)
                .await?;
        if admin_count.0 <= 1 {
            return Err(AppError::bad_request("cannot delete the last admin"));
        }
    }

    sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /api/users/:id/dashboard
///
/// A user may always view their own dashboard; managers and admins may view
/// anyone's.
pub async fn dashboard(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Dashboard>> {
    if id != user.id && !user.role.can_manage() {
        return Err(AppError::Forbidden);
    }

    let profile = sqlx::query_as::<_, UserProfile>(
        "SELECT id, email, role, created_at FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let completions = sqlx::query_as::<_, CompletionRecord>(
        "SELECT c.training_id, t.title, c.completed_at, c.score \
         FROM user_training_completions c \
         JOIN trainings t ON t.id = c.training_id \
         WHERE c.user_id = $1 \
         ORDER BY c.completed_at DESC",
    )
    .bind(id)
    .fetch_all(&state.db)
    .await?;

    let certifications = certification_statuses(&state, id).await?;

    Ok(Json(Dashboard {
        user: profile,
        completions,
        certifications,
    }))
}

/// Raw row used to compute certification status in Rust (keeps date math out
/// of SQL and consistent across endpoints).
#[derive(Debug, FromRow)]
struct CertRow {
    certification_id: Uuid,
    name: String,
    issuing_body: String,
    issued_date: NaiveDate,
    expiry_date: NaiveDate,
    document_url: Option<String>,
}

/// Compute the list of certification statuses for a single user, including the
/// days-to-expiry and a textual status (valid / expiring_soon / expired).
pub async fn certification_statuses(
    state: &AppState,
    user_id: Uuid,
) -> AppResult<Vec<CertificationStatus>> {
    let rows = sqlx::query_as::<_, CertRow>(
        "SELECT uc.certification_id, c.name, c.issuing_body, uc.issued_date, \
                uc.expiry_date, uc.document_url \
         FROM user_certifications uc \
         JOIN certifications c ON c.id = uc.certification_id \
         WHERE uc.user_id = $1 \
         ORDER BY uc.expiry_date ASC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let today = Utc::now().date_naive();
    Ok(rows
        .into_iter()
        .map(|r| {
            let days_to_expiry = (r.expiry_date - today).num_days();
            let status = if days_to_expiry < 0 {
                "expired"
            } else if days_to_expiry <= EXPIRY_WARNING_DAYS {
                "expiring_soon"
            } else {
                "valid"
            };
            CertificationStatus {
                certification_id: r.certification_id,
                name: r.name,
                issuing_body: r.issuing_body,
                issued_date: r.issued_date,
                expiry_date: r.expiry_date,
                document_url: r.document_url,
                days_to_expiry,
                status: status.to_string(),
            }
        })
        .collect())
}
