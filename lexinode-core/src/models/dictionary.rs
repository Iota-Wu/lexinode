use super::enums::MorphemeType;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Morpheme {
    pub id: Uuid,
    pub meaning: String,
    #[sqlx(rename = "type")]
    pub r#type: MorphemeType,
    pub variations: Vec<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Word {
    pub id: Uuid,
    pub spelling: String,
    pub phonetic: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WordComponent {
    pub word_id: Uuid,
    pub morpheme_id: Uuid,
    pub position: i16,
    pub surface_form: String,
}
