use axum::extract::{Path, State};
use axum::Json;
use chrono::{NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{CertificationStatus, CompletionRecord, Dashboard, UserProfile};
use crate::state::AppState;

/// Number of days before expiry at which a certification is flagged.
pub const EXPIRY_WARNING_DAYS: i64 = 30;

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
