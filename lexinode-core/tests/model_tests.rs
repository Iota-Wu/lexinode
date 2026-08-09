use lexinode_core::models::*;
use sqlx::PgPool;
use pgvector::Vector;
use chrono::{Utc, Duration};

// =====================================================================
// morphemes
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_create_morpheme(pool: PgPool) -> sqlx::Result<()> {
    let morpheme = sqlx::query_as!(
        Morpheme,
        r#"
        INSERT INTO morphemes (meaning, type, variations)
        VALUES ($1, $2, $3)
        RETURNING id, meaning, type as "type: MorphemeType", variations
        "#,
        "walk, move",
        MorphemeType::Root as MorphemeType,
        &vec!["ced".to_owned(), "ceed".to_owned(), "cede".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(morpheme.meaning, "walk, move");
    assert_eq!(morpheme.r#type, MorphemeType::Root);
    assert_eq!(morpheme.variations.len(), 3);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_morpheme_prefix_and_suffix_types(pool: PgPool) -> sqlx::Result<()> {
    let prefix = sqlx::query_as!(
        Morpheme,
        r#"
        INSERT INTO morphemes (meaning, type, variations)
        VALUES ($1, $2, $3)
        RETURNING id, meaning, type as "type: MorphemeType", variations
        "#,
        "before",
        MorphemeType::Prefix as MorphemeType,
        &vec!["pre".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(prefix.r#type, MorphemeType::Prefix);

    let suffix = sqlx::query_as!(
        Morpheme,
        r#"
        INSERT INTO morphemes (meaning, type, variations)
        VALUES ($1, $2, $3)
        RETURNING id, meaning, type as "type: MorphemeType", variations
        "#,
        "state or quality of",
        MorphemeType::Suffix as MorphemeType,
        &vec!["ness".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(suffix.r#type, MorphemeType::Suffix);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_find_morpheme_by_variation(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query_as!(
        Morpheme,
        r#"
        INSERT INTO morphemes (meaning, type, variations)
        VALUES ($1, $2, $3)
        RETURNING id, meaning, type as "type: MorphemeType", variations
        "#,
        "walk, move",
        MorphemeType::Root as MorphemeType,
        &vec!["ced".to_owned(), "ceed".to_owned(), "cede".to_owned(), "cess".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    // GIN index on variations should support finding a morpheme by any surface form
    let found = sqlx::query_as!(
        Morpheme,
        r#"
        SELECT id, meaning, type as "type: MorphemeType", variations
        FROM morphemes
        WHERE $1 = ANY(variations)
        "#,
        "cess"
    )
    .fetch_optional(&pool)
    .await?;

    assert!(found.is_some());
    assert_eq!(found.unwrap().meaning, "walk, move");

    Ok(())
}

// =====================================================================
// words
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_create_word(pool: PgPool) -> sqlx::Result<()> {
    let word = sqlx::query_as!(
        Word,
        r#"
        INSERT INTO words (spelling, phonetic)
        VALUES ($1, $2)
        RETURNING id, spelling, phonetic
        "#,
        "procedure",
        Some("/prəˈsiː.dʒɚ/".to_owned())
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(word.spelling, "procedure");
    assert_eq!(word.phonetic.as_deref(), Some("/prəˈsiː.dʒɚ/"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_word_without_phonetic(pool: PgPool) -> sqlx::Result<()> {
    let word = sqlx::query_as!(
        Word,
        r#"
        INSERT INTO words (spelling, phonetic)
        VALUES ($1, $2)
        RETURNING id, spelling, phonetic
        "#,
        "onomatopoeia",
        None::<String>
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(word.spelling, "onomatopoeia");
    assert!(word.phonetic.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_word_spelling_must_be_unique(pool: PgPool) -> sqlx::Result<()> {
    sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "duplicate"
    )
    .fetch_one(&pool)
    .await?;

    let second_insert = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "duplicate"
    )
    .fetch_one(&pool)
    .await;

    assert!(second_insert.is_err(), "expected unique constraint violation on spelling");

    Ok(())
}

// =====================================================================
// word_components
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_link_word_to_morpheme_component(pool: PgPool) -> sqlx::Result<()> {
    let morpheme = sqlx::query_as!(
        Morpheme,
        r#"
        INSERT INTO morphemes (meaning, type, variations)
        VALUES ($1, $2, $3)
        RETURNING id, meaning, type as "type: MorphemeType", variations
        "#,
        "walk, move",
        MorphemeType::Root as MorphemeType,
        &vec!["ced".to_owned(), "ceed".to_owned(), "cede".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    let word = sqlx::query_as!(
        Word,
        r#"
        INSERT INTO words (spelling, phonetic)
        VALUES ($1, $2)
        RETURNING id, spelling, phonetic
        "#,
        "procedure",
        Some("/prəˈsiː.dʒɚ/".to_owned())
    )
    .fetch_one(&pool)
    .await?;

    let component = sqlx::query_as!(
        WordComponent,
        r#"
        INSERT INTO word_components (word_id, morpheme_id, position, surface_form)
        VALUES ($1, $2, $3, $4)
        RETURNING word_id, morpheme_id, position, surface_form
        "#,
        word.id,
        morpheme.id,
        1i16,
        "ced"
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(component.word_id, word.id);
    assert_eq!(component.morpheme_id, morpheme.id);
    assert_eq!(component.surface_form, "ced");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_word_with_multiple_morpheme_components_in_order(pool: PgPool) -> sqlx::Result<()> {
    let prefix = sqlx::query_scalar!(
        r#"INSERT INTO morphemes (meaning, type, variations) VALUES ($1, $2, $3) RETURNING id"#,
        "forward",
        MorphemeType::Prefix as MorphemeType,
        &vec!["pro".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    let root = sqlx::query_scalar!(
        r#"INSERT INTO morphemes (meaning, type, variations) VALUES ($1, $2, $3) RETURNING id"#,
        "walk, move",
        MorphemeType::Root as MorphemeType,
        &vec!["ced".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    let suffix = sqlx::query_scalar!(
        r#"INSERT INTO morphemes (meaning, type, variations) VALUES ($1, $2, $3) RETURNING id"#,
        "act or process of",
        MorphemeType::Suffix as MorphemeType,
        &vec!["ure".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "procedure"
    )
    .fetch_one(&pool)
    .await?;

    for (morpheme_id, position, surface_form) in [
        (prefix, 0i16, "pro"),
        (root, 1i16, "ced"),
        (suffix, 2i16, "ure"),
    ] {
        sqlx::query!(
            r#"INSERT INTO word_components (word_id, morpheme_id, position, surface_form) VALUES ($1, $2, $3, $4)"#,
            word_id,
            morpheme_id,
            position,
            surface_form
        )
        .execute(&pool)
        .await?;
    }

    let components = sqlx::query_as!(
        WordComponent,
        r#"
        SELECT word_id, morpheme_id, position, surface_form
        FROM word_components
        WHERE word_id = $1
        ORDER BY position ASC
        "#,
        word_id
    )
    .fetch_all(&pool)
    .await?;

    assert_eq!(components.len(), 3);
    assert_eq!(components[0].surface_form, "pro");
    assert_eq!(components[1].surface_form, "ced");
    assert_eq!(components[2].surface_form, "ure");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deleting_word_cascades_to_word_components(pool: PgPool) -> sqlx::Result<()> {
    let morpheme_id = sqlx::query_scalar!(
        r#"INSERT INTO morphemes (meaning, type, variations) VALUES ($1, $2, $3) RETURNING id"#,
        "walk, move",
        MorphemeType::Root as MorphemeType,
        &vec!["ced".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "procedure"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO word_components (word_id, morpheme_id, position, surface_form) VALUES ($1, $2, $3, $4)"#,
        word_id,
        morpheme_id,
        0i16,
        "ced"
    )
    .execute(&pool)
    .await?;

    sqlx::query!(r#"DELETE FROM words WHERE id = $1"#, word_id)
        .execute(&pool)
        .await?;

    let remaining = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM word_components WHERE word_id = $1"#,
        word_id
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(remaining, Some(0));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deleting_morpheme_in_use_is_restricted(pool: PgPool) -> sqlx::Result<()> {
    let morpheme_id = sqlx::query_scalar!(
        r#"INSERT INTO morphemes (meaning, type, variations) VALUES ($1, $2, $3) RETURNING id"#,
        "walk, move",
        MorphemeType::Root as MorphemeType,
        &vec!["ced".to_owned()][..]
    )
    .fetch_one(&pool)
    .await?;

    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "procedure"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO word_components (word_id, morpheme_id, position, surface_form) VALUES ($1, $2, $3, $4)"#,
        word_id,
        morpheme_id,
        0i16,
        "ced"
    )
    .execute(&pool)
    .await?;

    // morpheme_id REFERENCES ... ON DELETE RESTRICT, so this must fail
    let delete_result = sqlx::query!(r#"DELETE FROM morphemes WHERE id = $1"#, morpheme_id)
        .execute(&pool)
        .await;

    assert!(delete_result.is_err(), "expected RESTRICT to prevent deleting a referenced morpheme");

    Ok(())
}

// =====================================================================
// definitions / vector embeddings
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_create_definition_with_vector_embedding(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "method"
    )
    .fetch_one(&pool)
    .await?;

    let embedding_input = Vector::from(vec![0.15, -0.32, 0.88]);

    let def = sqlx::query_as!(
        Definition,
        r#"
        INSERT INTO definitions (word_id, part_of_speech, meaning, embedding)
        VALUES ($1, $2, $3, $4)
        RETURNING id, word_id, part_of_speech as "part_of_speech: PartOfSpeechType", meaning, embedding as "embedding: Vector"
        "#,
        word_id,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "a particular form of procedure for accomplishing or approaching something",
        embedding_input as Vector
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(def.part_of_speech, PartOfSpeechType::Noun);
    assert!(def.embedding.is_some());

    let retrieved_vec = def.embedding.unwrap();
    assert_eq!(retrieved_vec.as_slice(), &[0.15, -0.32, 0.88]);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_definition_without_embedding(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "serendipity"
    )
    .fetch_one(&pool)
    .await?;

    let def = sqlx::query_as!(
        Definition,
        r#"
        INSERT INTO definitions (word_id, part_of_speech, meaning, embedding)
        VALUES ($1, $2, $3, $4)
        RETURNING id, word_id, part_of_speech as "part_of_speech: PartOfSpeechType", meaning, embedding as "embedding: Vector"
        "#,
        word_id,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "the occurrence of events by chance in a happy way",
        None::<Vector> as Option<Vector>
    )
    .fetch_one(&pool)
    .await?;

    assert!(def.embedding.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_word_can_have_multiple_definitions_across_parts_of_speech(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "light"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3)"#,
        word_id,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "the natural agent that stimulates sight"
    )
    .execute(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3)"#,
        word_id,
        PartOfSpeechType::Adjective as PartOfSpeechType,
        "not heavy; of little weight"
    )
    .execute(&pool)
    .await?;

    let definitions = sqlx::query_as!(
        Definition,
        r#"
        SELECT id, word_id, part_of_speech as "part_of_speech: PartOfSpeechType", meaning, embedding as "embedding: Vector"
        FROM definitions
        WHERE word_id = $1
        ORDER BY part_of_speech
        "#,
        word_id
    )
    .fetch_all(&pool)
    .await?;

    assert_eq!(definitions.len(), 2);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deleting_word_cascades_to_definitions(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "method"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3)"#,
        word_id,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "a way of doing something"
    )
    .execute(&pool)
    .await?;

    sqlx::query!(r#"DELETE FROM words WHERE id = $1"#, word_id)
        .execute(&pool)
        .await?;

    let remaining = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM definitions WHERE word_id = $1"#,
        word_id
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(remaining, Some(0));

    Ok(())
}

// =====================================================================
// synonyms
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_link_synonym_between_definitions(pool: PgPool) -> sqlx::Result<()> {
    let word_a = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "procedure"
    )
    .fetch_one(&pool)
    .await?;

    let word_b = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "method"
    )
    .fetch_one(&pool)
    .await?;

    let def_a = sqlx::query_scalar!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3) RETURNING id"#,
        word_a,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "a process or method for doing something"
    )
    .fetch_one(&pool)
    .await?;

    let def_b = sqlx::query_scalar!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3) RETURNING id"#,
        word_b,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "a particular form of procedure"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO synonyms (source_def_id, target_def_id, similarity_score) VALUES ($1, $2, $3)"#,
        def_a,
        def_b,
        0.92f64
    )
    .execute(&pool)
    .await?;

    let score: f64 = sqlx::query_scalar!(
        r#"SELECT similarity_score FROM synonyms WHERE source_def_id = $1 AND target_def_id = $2"#,
        def_a,
        def_b
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(score, 0.92);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deleting_definition_cascades_to_synonyms(pool: PgPool) -> sqlx::Result<()> {
    let word_a = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "procedure"
    )
    .fetch_one(&pool)
    .await?;

    let word_b = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "method"
    )
    .fetch_one(&pool)
    .await?;

    let def_a = sqlx::query_scalar!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3) RETURNING id"#,
        word_a,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "a process or method for doing something"
    )
    .fetch_one(&pool)
    .await?;

    let def_b = sqlx::query_scalar!(
        r#"INSERT INTO definitions (word_id, part_of_speech, meaning) VALUES ($1, $2, $3) RETURNING id"#,
        word_b,
        PartOfSpeechType::Noun as PartOfSpeechType,
        "a particular form of procedure"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO synonyms (source_def_id, target_def_id, similarity_score) VALUES ($1, $2, $3)"#,
        def_a,
        def_b,
        0.92f64
    )
    .execute(&pool)
    .await?;

    sqlx::query!(r#"DELETE FROM definitions WHERE id = $1"#, def_a)
        .execute(&pool)
        .await?;

    let remaining = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM synonyms WHERE source_def_id = $1"#,
        def_a
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(remaining, Some(0));

    Ok(())
}

// =====================================================================
// sources
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_create_source(pool: PgPool) -> sqlx::Result<()> {
    let source = sqlx::query_as!(
        Source,
        r#"
        INSERT INTO sources (title, url, content_hash, type)
        VALUES ($1, $2, $3, $4)
        RETURNING id, title, url, content_hash, type as "type: SourceType", created_at
        "#,
        Some("Rust Documentation".to_owned()),
        Some("https://doc.rust-lang.org".to_owned()),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        SourceType::Web as SourceType
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(source.r#type, SourceType::Web);
    assert_eq!(source.title.as_deref(), Some("Rust Documentation"));

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_clipboard_source_without_url(pool: PgPool) -> sqlx::Result<()> {
    let source = sqlx::query_as!(
        Source,
        r#"
        INSERT INTO sources (title, url, content_hash, type)
        VALUES ($1, $2, $3, $4)
        RETURNING id, title, url, content_hash, type as "type: SourceType", created_at
        "#,
        None::<String>,
        None::<String>,
        "d41d8cd98f00b204e9800998ecf8427ed41d8cd98f00b204e9800998ecf842",
        SourceType::Clipboard as SourceType
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(source.r#type, SourceType::Clipboard);
    assert!(source.url.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_source_content_hash_must_be_unique(pool: PgPool) -> sqlx::Result<()> {
    let hash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    sqlx::query!(
        r#"INSERT INTO sources (title, content_hash, type) VALUES ($1, $2, $3)"#,
        Some("First".to_owned()),
        hash,
        SourceType::Web as SourceType
    )
    .execute(&pool)
    .await?;

    let second_insert = sqlx::query!(
        r#"INSERT INTO sources (title, content_hash, type) VALUES ($1, $2, $3)"#,
        Some("Duplicate".to_owned()),
        hash,
        SourceType::Web as SourceType
    )
    .execute(&pool)
    .await;

    assert!(second_insert.is_err(), "expected unique constraint violation on content_hash");

    Ok(())
}

// =====================================================================
// user_items / FSRS metrics
// =====================================================================

#[sqlx::test(migrations = "./migrations")]
async fn test_create_user_item_with_default_fsrs_values(pool: PgPool) -> sqlx::Result<()> {
    let source = sqlx::query_as!(
        Source,
        r#"
        INSERT INTO sources (title, url, content_hash, type)
        VALUES ($1, $2, $3, $4)
        RETURNING id, title, url, content_hash, type as "type: SourceType", created_at
        "#,
        Some("Rust Documentation".to_owned()),
        Some("https://doc.rust-lang.org".to_owned()),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        SourceType::Web as SourceType
    )
    .fetch_one(&pool)
    .await?;

    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "ownership"
    )
    .fetch_one(&pool)
    .await?;

    let due_date = Utc::now() + Duration::days(1);

    let user_item = sqlx::query_as!(
        UserItem,
        r#"
        INSERT INTO user_items (word_id, source_id, context_snippet, due_at)
        VALUES ($1, $2, $3, $4)
        RETURNING
            id, word_id, source_id, context_snippet,
            stability, difficulty, reps,
            last_review_at, due_at, created_at
        "#,
        word_id,
        source.id,
        "Ownership is a set of rules that govern how a Rust program manages memory.",
        due_date
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(user_item.stability, 0.0);
    assert_eq!(user_item.difficulty, 0.0);
    assert_eq!(user_item.reps, 0);
    assert!(user_item.last_review_at.is_none());
    assert!(user_item.due_at.is_some());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_user_item_without_due_date(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "borrow"
    )
    .fetch_one(&pool)
    .await?;

    let user_item = sqlx::query_as!(
        UserItem,
        r#"
        INSERT INTO user_items (word_id, source_id, context_snippet, due_at)
        VALUES ($1, $2, $3, $4)
        RETURNING
            id, word_id, source_id, context_snippet,
            stability, difficulty, reps,
            last_review_at, due_at, created_at
        "#,
        word_id,
        None::<uuid::Uuid>,
        "You can borrow a value without taking ownership of it.",
        None::<chrono::DateTime<Utc>>
    )
    .fetch_one(&pool)
    .await?;

    assert!(user_item.due_at.is_none());
    assert!(user_item.source_id.is_none());

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_query_overdue_user_items_excludes_not_yet_due(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "ownership"
    )
    .fetch_one(&pool)
    .await?;

    let now = Utc::now();

    let overdue_item = sqlx::query_scalar!(
        r#"INSERT INTO user_items (word_id, context_snippet, due_at) VALUES ($1, $2, $3) RETURNING id"#,
        word_id,
        "Due yesterday, should show up as overdue.",
        now - Duration::days(1)
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO user_items (word_id, context_snippet, due_at) VALUES ($1, $2, $3)"#,
        word_id,
        "Due next week, should NOT show up as overdue.",
        now + Duration::days(7)
    )
    .execute(&pool)
    .await?;

    let overdue_items = sqlx::query_as!(
        UserItem,
        r#"
        SELECT
            id, word_id, source_id, context_snippet,
            stability, difficulty, reps,
            last_review_at, due_at, created_at
        FROM user_items
        WHERE due_at <= $1
        "#,
        now
    )
    .fetch_all(&pool)
    .await?;

    assert_eq!(overdue_items.len(), 1);
    assert_eq!(overdue_items[0].id, overdue_item);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_query_overdue_user_items_includes_exact_boundary(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "ownership"
    )
    .fetch_one(&pool)
    .await?;

    let due_at = Utc::now();

    let item_id = sqlx::query_scalar!(
        r#"INSERT INTO user_items (word_id, context_snippet, due_at) VALUES ($1, $2, $3) RETURNING id"#,
        word_id,
        "Due exactly now, boundary case for <= comparison.",
        due_at
    )
    .fetch_one(&pool)
    .await?;

    // Query using the exact same timestamp; "due_at <= $1" should include it.
    let overdue_items = sqlx::query_as!(
        UserItem,
        r#"
        SELECT
            id, word_id, source_id, context_snippet,
            stability, difficulty, reps,
            last_review_at, due_at, created_at
        FROM user_items
        WHERE due_at <= $1
        "#,
        due_at
    )
    .fetch_all(&pool)
    .await?;

    assert_eq!(overdue_items.len(), 1);
    assert_eq!(overdue_items[0].id, item_id);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_query_overdue_user_items_excludes_null_due_at(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "ownership"
    )
    .fetch_one(&pool)
    .await?;

    // No due_at set at all (e.g. a freshly captured item not yet scheduled).
    sqlx::query!(
        r#"INSERT INTO user_items (word_id, context_snippet, due_at) VALUES ($1, $2, $3)"#,
        word_id,
        "Not scheduled yet.",
        None::<chrono::DateTime<Utc>>
    )
    .execute(&pool)
    .await?;

    let overdue_items = sqlx::query_as!(
        UserItem,
        r#"
        SELECT
            id, word_id, source_id, context_snippet,
            stability, difficulty, reps,
            last_review_at, due_at, created_at
        FROM user_items
        WHERE due_at <= $1
        "#,
        Utc::now() + Duration::days(365)
    )
    .fetch_all(&pool)
    .await?;

    // NULL due_at should never satisfy "due_at <= $1" in SQL's three-valued logic
    assert_eq!(overdue_items.len(), 0);

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deleting_source_sets_user_item_source_id_null(pool: PgPool) -> sqlx::Result<()> {
    let source_id = sqlx::query_scalar!(
        r#"INSERT INTO sources (title, content_hash, type) VALUES ($1, $2, $3) RETURNING id"#,
        Some("Some Source".to_owned()),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        SourceType::Web as SourceType
    )
    .fetch_one(&pool)
    .await?;

    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "ownership"
    )
    .fetch_one(&pool)
    .await?;

    let item_id = sqlx::query_scalar!(
        r#"INSERT INTO user_items (word_id, source_id, context_snippet) VALUES ($1, $2, $3) RETURNING id"#,
        word_id,
        source_id,
        "Snippet referencing a source that will be deleted."
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(r#"DELETE FROM sources WHERE id = $1"#, source_id)
        .execute(&pool)
        .await?;

    let item_source_id: Option<uuid::Uuid> = sqlx::query_scalar!(
        r#"SELECT source_id FROM user_items WHERE id = $1"#,
        item_id
    )
    .fetch_one(&pool)
    .await?;

    assert!(item_source_id.is_none(), "expected ON DELETE SET NULL to clear source_id");

    Ok(())
}

#[sqlx::test(migrations = "./migrations")]
async fn test_deleting_word_cascades_to_user_items(pool: PgPool) -> sqlx::Result<()> {
    let word_id = sqlx::query_scalar!(
        r#"INSERT INTO words (spelling) VALUES ($1) RETURNING id"#,
        "ownership"
    )
    .fetch_one(&pool)
    .await?;

    sqlx::query!(
        r#"INSERT INTO user_items (word_id, context_snippet) VALUES ($1, $2)"#,
        word_id,
        "This should disappear when the word is deleted."
    )
    .execute(&pool)
    .await?;

    sqlx::query!(r#"DELETE FROM words WHERE id = $1"#, word_id)
        .execute(&pool)
        .await?;

    let remaining = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM user_items WHERE word_id = $1"#,
        word_id
    )
    .fetch_one(&pool)
    .await?;

    assert_eq!(remaining, Some(0));

    Ok(())
}