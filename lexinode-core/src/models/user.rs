use super::enums::SourceType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Source {
    pub id: Uuid,
    pub title: Option<String>,
    pub url: Option<String>,
    pub content_hash: String,
    #[sqlx(rename = "type")]
    pub r#type: SourceType,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct UserItem {
    pub id: Uuid,
    pub word_id: Uuid,
    pub source_id: Option<Uuid>,
    pub context_snippet: String,

    // FSRS Metrics
    pub stability: f64,
    pub difficulty: f64,
    pub reps: i32,
    pub last_review_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,

    pub created_at: DateTime<Utc>,
}
