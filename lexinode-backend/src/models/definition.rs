use super::enums::PartOfSpeechType;
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Definition {
    pub id: Uuid,
    pub word_id: Uuid,
    pub part_of_speech: PartOfSpeechType,
    pub meaning: String,
    pub embedding: Option<Vector>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Synonym {
    pub source_def_id: Uuid,
    pub target_def_id: Uuid,
    pub similarity_score: f64,
}
