use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Manager,
    Learner,
}

impl Role {
    pub fn can_manage(&self) -> bool {
        matches!(self, Role::Admin | Role::Manager)
    }
    pub fn is_admin(&self) -> bool {
        matches!(self, Role::Admin)
    }
}

/// Public projection of a user (never includes the password hash).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Training {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub duration_minutes: i32,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Certification {
    pub id: Uuid,
    pub name: String,
    pub issuing_body: String,
    pub validity_months: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CompletionRecord {
    pub training_id: Uuid,
    pub title: String,
    pub completed_at: DateTime<Utc>,
    pub score: Option<i32>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CertificationStatus {
    pub certification_id: Uuid,
    pub name: String,
    pub issuing_body: String,
    pub issued_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub document_url: Option<String>,
    pub days_to_expiry: i64,
    /// "valid" | "expiring_soon" | "expired"
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dashboard {
    pub user: UserProfile,
    pub completions: Vec<CompletionRecord>,
    pub certifications: Vec<CertificationStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComplianceEntry {
    pub user: UserProfile,
    pub certifications: Vec<CertificationStatus>,
}

/// A scheduled training session, enriched with the training title, the current
/// number of enrolled people, and whether the requesting user is enrolled.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SessionView {
    pub id: Uuid,
    pub training_id: Uuid,
    pub training_title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub location: String,
    pub instructor: Option<String>,
    pub capacity: Option<i32>,
    pub enrolled_count: i64,
    pub enrolled: bool,
}

/// A single person's enrollment in a session (manager view).
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct SessionEnrollee {
    pub user_id: Uuid,
    pub email: String,
    pub status: String,
    pub enrolled_at: DateTime<Utc>,
}
