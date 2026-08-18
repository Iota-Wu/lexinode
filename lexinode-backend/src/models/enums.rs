use serde::{Deserialize, Serialize};
use sqlx::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type, Serialize, Deserialize)]
#[sqlx(type_name = "morpheme_type", rename_all = "lowercase")]
pub enum MorphemeType {
    Prefix,
    Root,
    Suffix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "source_type", rename_all = "lowercase")]
pub enum SourceType {
    Web,
    Clipboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "part_of_speech_type", rename_all = "snake_case")]
pub enum PartOfSpeechType {
    Noun,
    Verb,
    Adjective,
    Adverb,
    Pronoun,
    Preposition,
    Conjunction,
    Interjection,
    Phase,
}
