-- Add migration script here
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TYPE morpheme_type AS ENUM ('prefix', 'root', 'suffix');
CREATE TYPE source_type AS ENUM ('web', 'clipboard');
CREATE TYPE part_of_speech_type AS ENUM (
    'noun', 'verb', 'adjective', 'adverb', 'pronoun', 
    'preposition', 'conjunction', 'interjection', 'phrase'
);

-- E.g. For the morpheme "ced":
-- (id: <UUID>, meaning: 'walk, move', type: 'root', variations: ['ced', 'ceed', 'cede', 'cess'])
CREATE TABLE morphemes (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    meaning TEXT NOT NULL,
    type morpheme_type NOT NULL,
    variations TEXT[] NOT NULL
);

CREATE INDEX idx_morphemes_variations ON morphemes USING GIN (variations);

-- E.g. For the word "procedure":
-- (id: <UUID>, spelling: 'procedure', phonetic: '/prəˈsiː.dʒɚ/')
CREATE TABLE words (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    spelling TEXT NOT NULL UNIQUE,
    phonetic TEXT
);

-- E.g. For the word "procedure":
-- (word_id: <UUID>, morpheme_id: <UUID for 'pro'>, position: 0, surface_form: 'pro')
-- (word_id: <UUID>, morpheme_id: <UUID for 'ced'>,  position: 1, surface_form: 'ced')
-- (word_id: <UUID>, morpheme_id: <UUID for 'ure'>, position: 2, surface_form: 'ure')
CREATE TABLE word_components (
    word_id UUID NOT NULL REFERENCES words(id) ON DELETE CASCADE,
    morpheme_id UUID NOT NULL REFERENCES morphemes(id) ON DELETE RESTRICT,
    position SMALLINT NOT NULL,
    surface_form TEXT NOT NULL,    -- Record the actual surface form used (e.g. "pli")
    PRIMARY KEY (word_id, morpheme_id, position)
);

CREATE INDEX idx_word_components_morpheme ON word_components (morpheme_id);

-- E.g. For the word "procedure":
-- (id: <UUID>, word_id: <UUID for 'procedure'>, part_of_speech: 'noun',
-- meaning: 'a process or method for doing something', embedding: <vector>)
CREATE TABLE definitions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    word_id UUID NOT NULL REFERENCES words(id) ON DELETE CASCADE,
    part_of_speech part_of_speech_type NOT NULL,
    meaning TEXT NOT NULL,
    -- The embedding vector may be changable while using different embedding models
    -- Thus, we have to create index by `hnsw (embedding vector_cosine_ops)` in lexinode-cli manually
    embedding vector
);

-- E.g. For the words "procedure" and "method":
-- (source_def_id: <UUID for 'procedure'>, target_def_id: <UUID for 'method'>, similarity_score: <float>)
CREATE TABLE synonyms (
    source_def_id UUID NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    target_def_id UUID NOT NULL REFERENCES definitions(id) ON DELETE CASCADE,
    similarity_score FLOAT NOT NULL,
    PRIMARY KEY (source_def_id, target_def_id)
);

-- E.g. For the word "procedure":
-- (id: <UUID for 'procedure'>, title: <title of the source>, url: <URL of the source>,
-- content_hash: <content hash of the source>, type: <source type>, created_at: <timestamp>)
CREATE TABLE sources (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    title TEXT,
    url TEXT,
    content_hash VARCHAR(64) UNIQUE NOT NULL,
    type source_type NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE user_items (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    word_id UUID NOT NULL REFERENCES words(id) ON DELETE CASCADE,
    source_id UUID REFERENCES sources(id) ON DELETE SET NULL,
    context_snippet TEXT NOT NULL,
    
    stability FLOAT NOT NULL DEFAULT 0,
    difficulty FLOAT NOT NULL DEFAULT 0,
    reps INT NOT NULL DEFAULT 0,
    last_review_at TIMESTAMPTZ,
    due_at TIMESTAMPTZ,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_user_items_due_at ON user_items (due_at);