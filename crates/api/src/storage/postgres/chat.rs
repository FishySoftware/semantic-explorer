use anyhow::Result;
use sqlx::types::chrono::Utc;
use sqlx::{Pool, Postgres};
use std::time::Instant;
use uuid::Uuid;

use crate::chat::models::{ChatMessage, ChatSession, CreateChatSessionRequest, RetrievedDocument};
use semantic_explorer_core::observability::record_database_query;

const ENSURE_USER_QUERY: &str = r#"
    INSERT INTO users (username, created_at)
    VALUES ($1, NOW())
    ON CONFLICT (username) DO NOTHING
"#;

const CREATE_SESSION_QUERY: &str = r#"
    INSERT INTO chat_sessions (session_id, owner, embedded_dataset_id, llm_id, title, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
    RETURNING session_id, owner, embedded_dataset_id, llm_id, title, created_at, updated_at
"#;

const GET_SESSION_QUERY: &str = r#"
    SELECT session_id, owner, embedded_dataset_id, llm_id, title, created_at, updated_at
    FROM chat_sessions
    WHERE session_id = $1 AND owner = $2
"#;

const GET_SESSIONS_QUERY: &str = r#"
    SELECT session_id, owner, embedded_dataset_id, llm_id, title, created_at, updated_at
    FROM chat_sessions
    WHERE owner = $1
    ORDER BY updated_at DESC
"#;

const DELETE_SESSION_QUERY: &str = r#"
    DELETE FROM chat_sessions WHERE session_id = $1 AND owner = $2
"#;

const CREATE_MESSAGE_QUERY: &str = r#"
    INSERT INTO chat_messages (session_id, role, content, documents_retrieved, created_at)
    VALUES ($1, $2, $3, $4, NOW())
    RETURNING message_id, session_id, role, content, documents_retrieved, created_at
"#;

const GET_MESSAGES_QUERY: &str = r#"
    SELECT message_id, session_id, role, content, documents_retrieved, created_at
    FROM chat_messages
    WHERE session_id = $1
    ORDER BY created_at ASC
"#;

const GET_LLM_DETAILS_QUERY: &str = r#"
    SELECT name, provider, base_url, config->>'model' as model, api_key
    FROM llms
    WHERE llm_id = $1
"#;

const BATCH_INSERT_RETRIEVED_DOCUMENTS_QUERY: &str = r#"
    INSERT INTO chat_message_retrieved_documents (message_id, document_id, text, similarity_score, item_title, created_at)
    SELECT $1, unnest($2::text[]), unnest($3::text[]), unnest($4::float4[]), unnest($5::text[]), NOW()
"#;

const GET_RETRIEVED_DOCUMENTS_QUERY: &str = r#"
    SELECT document_id, text, similarity_score, item_title
    FROM chat_message_retrieved_documents
    WHERE message_id = $1
    ORDER BY similarity_score DESC
"#;

#[tracing::instrument(name = "database.create_chat_session", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", owner = %owner))]
pub(crate) async fn create_chat_session(
    pool: &Pool<Postgres>,
    owner: &str,
    request: &CreateChatSessionRequest,
) -> Result<ChatSession> {
    let start = Instant::now();
    let session_id = Uuid::new_v4().to_string();

    // Generate default title if not provided
    let title = request.title.clone().unwrap_or_else(|| {
        let now = Utc::now();
        format!("chat-session-{}", now.format("%Y%m%d-%H%M%S"))
    });

    // Ensure user exists in users table (required by foreign key constraint)
    sqlx::query(ENSURE_USER_QUERY)
        .bind(owner)
        .execute(pool)
        .await?;

    let result = sqlx::query_as::<_, ChatSession>(CREATE_SESSION_QUERY)
        .bind(&session_id)
        .bind(owner)
        .bind(request.embedded_dataset_id)
        .bind(request.llm_id)
        .bind(&title)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "chat_sessions", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_chat_session", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_chat_session(
    pool: &Pool<Postgres>,
    session_id: &str,
    owner: &str,
) -> Result<ChatSession> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, ChatSession>(GET_SESSION_QUERY)
        .bind(session_id)
        .bind(owner)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "chat_sessions", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_chat_sessions", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_chat_sessions(
    pool: &Pool<Postgres>,
    owner: &str,
) -> Result<Vec<ChatSession>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, ChatSession>(GET_SESSIONS_QUERY)
        .bind(owner)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "chat_sessions", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.delete_chat_session", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", owner = %owner))]
pub(crate) async fn delete_chat_session(
    pool: &Pool<Postgres>,
    session_id: &str,
    owner: &str,
) -> Result<()> {
    let start = Instant::now();
    let result = sqlx::query(DELETE_SESSION_QUERY)
        .bind(session_id)
        .bind(owner)
        .execute(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("DELETE", "chat_sessions", duration, success);

    result?;
    Ok(())
}

#[tracing::instrument(name = "database.add_chat_message", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT"))]
pub(crate) async fn add_chat_message(
    pool: &Pool<Postgres>,
    session_id: &str,
    role: &str,
    content: &str,
    documents_retrieved: Option<i32>,
) -> Result<ChatMessage> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, ChatMessage>(CREATE_MESSAGE_QUERY)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(documents_retrieved)
        .fetch_one(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("INSERT", "chat_messages", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_chat_messages", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_chat_messages(
    pool: &Pool<Postgres>,
    session_id: &str,
) -> Result<Vec<ChatMessage>> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, ChatMessage>(GET_MESSAGES_QUERY)
        .bind(session_id)
        .fetch_all(pool)
        .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "chat_messages", duration, success);

    Ok(result?)
}

#[tracing::instrument(name = "database.get_llm_details", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_llm_details(
    pool: &Pool<Postgres>,
    llm_id: i32,
) -> Result<(String, String, String, String, Option<String>)> {
    let start = Instant::now();
    let result = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
        GET_LLM_DETAILS_QUERY,
    )
    .bind(llm_id)
    .fetch_optional(pool)
    .await;

    let duration = start.elapsed().as_secs_f64();
    let success = result.is_ok();
    record_database_query("SELECT", "llms", duration, success);

    result?.ok_or_else(|| anyhow::anyhow!("LLM not found"))
}

#[tracing::instrument(name = "database.store_retrieved_documents", skip(pool, documents), fields(database.system = "postgresql", database.operation = "INSERT", count = documents.len()))]
pub(crate) async fn store_retrieved_documents(
    pool: &Pool<Postgres>,
    message_id: i32,
    documents: &[RetrievedDocument],
) -> Result<()> {
    let start = Instant::now();

    if documents.is_empty() {
        return Ok(());
    }

    // Batch insert using UNNEST for better performance
    // Process in chunks to avoid parameter limits
    //TODO: make chunk size configurable if needed
    const BATCH_SIZE: usize = 500;

    for chunk in documents.chunks(BATCH_SIZE) {
        let mut document_ids: Vec<Option<String>> = Vec::with_capacity(chunk.len());
        let mut texts: Vec<String> = Vec::with_capacity(chunk.len());
        let mut scores: Vec<f32> = Vec::with_capacity(chunk.len());
        let mut item_titles: Vec<Option<String>> = Vec::with_capacity(chunk.len());

        for doc in chunk {
            document_ids.push(doc.document_id.clone());
            texts.push(doc.text.clone());
            scores.push(doc.similarity_score);
            item_titles.push(doc.item_title.clone());
        }

        // Use UNNEST for efficient batch insert
        sqlx::query(BATCH_INSERT_RETRIEVED_DOCUMENTS_QUERY)
            .bind(message_id)
            .bind(&document_ids)
            .bind(&texts)
            .bind(&scores)
            .bind(&item_titles)
            .execute(pool)
            .await?;
    }

    let duration = start.elapsed().as_secs_f64();
    record_database_query("INSERT", "chat_message_retrieved_documents", duration, true);

    Ok(())
}

#[tracing::instrument(name = "database.get_retrieved_documents", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_retrieved_documents(
    pool: &Pool<Postgres>,
    message_id: i32,
) -> Result<Vec<RetrievedDocument>> {
    let start = Instant::now();

    let result = sqlx::query_as::<_, (Option<String>, String, f32, Option<String>)>(
        GET_RETRIEVED_DOCUMENTS_QUERY,
    )
    .bind(message_id)
    .fetch_all(pool)
    .await?;

    let documents: Vec<RetrievedDocument> = result
        .into_iter()
        .map(
            |(document_id, text, similarity_score, item_title)| RetrievedDocument {
                document_id,
                text,
                similarity_score,
                item_title,
            },
        )
        .collect();

    let duration = start.elapsed().as_secs_f64();
    record_database_query("SELECT", "chat_message_retrieved_documents", duration, true);

    Ok(documents)
}
