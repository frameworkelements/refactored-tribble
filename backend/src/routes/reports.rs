use axum::extract::State;
use axum::Json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::AppResult;
use crate::models::{ComplianceEntry, UserProfile};
use crate::routes::users::{certification_statuses, EXPIRY_WARNING_DAYS};
use crate::state::AppState;

/// GET /api/reports/compliance — admin only.
///
/// Returns every user that has at least one certification which is overdue
/// (expired) or expiring within the warning window, along with the offending
/// certification records.
pub async fn compliance(
    State(state): State<AppState>,
    user: AuthUser,
) -> AppResult<Json<Vec<ComplianceEntry>>> {
    user.require_admin()?;

    // Candidate users: those holding a cert that is expired or expiring soon.
    let candidates = sqlx::query_as::<_, UserProfile>(
        "SELECT DISTINCT u.id, u.email, u.role, u.created_at \
         FROM users u \
         JOIN user_certifications uc ON uc.user_id = u.id \
         WHERE uc.expiry_date <= (now()::date + ($1 || ' days')::interval) \
         ORDER BY u.email ASC",
    )
    .bind(EXPIRY_WARNING_DAYS.to_string())
    .fetch_all(&state.db)
    .await?;

    let mut entries = Vec::with_capacity(candidates.len());
    for profile in candidates {
        let id: Uuid = profile.id;
        let all = certification_statuses(&state, id).await?;
        // Keep only the non-valid certs for the report.
        let flagged: Vec<_> = all
            .into_iter()
            .filter(|c| c.status != "valid")
            .collect();
        if !flagged.is_empty() {
            entries.push(ComplianceEntry {
                user: profile,
                certifications: flagged,
            });
        }
    }

    Ok(Json(entries))
}
