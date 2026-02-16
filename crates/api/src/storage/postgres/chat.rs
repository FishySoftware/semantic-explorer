use anyhow::Result;
use sqlx::types::chrono::{DateTime, Utc};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use uuid::Uuid;

use crate::chat::models::{
    ChatMessage, ChatSession, ChatSessions, CreateChatSessionRequest, RetrievedDocument,
};
use semantic_explorer_core::encryption::EncryptionService;

/// Helper struct for paginated queries that include total_count via COUNT(*) OVER()
#[derive(sqlx::FromRow)]
struct ChatSessionWithCount {
    pub session_id: String,
    pub owner_id: String,
    pub owner_display_name: String,
    pub embedded_dataset_id: i32,
    pub llm_id: i32,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub total_count: i64,
}

impl ChatSessionWithCount {
    fn into_parts(rows: Vec<Self>) -> (Vec<ChatSession>, i64) {
        let total_count = rows.first().map_or(0, |r| r.total_count);
        let sessions = rows
            .into_iter()
            .map(|r| ChatSession {
                session_id: r.session_id,
                owner_id: r.owner_id,
                owner_display_name: r.owner_display_name,
                embedded_dataset_id: r.embedded_dataset_id,
                llm_id: r.llm_id,
                title: r.title,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect();
        (sessions, total_count)
    }
}

const CREATE_SESSION_QUERY: &str = r#"
    INSERT INTO chat_sessions (session_id, owner_id, owner_display_name, embedded_dataset_id, llm_id, title, created_at, updated_at)
    VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
    RETURNING session_id, owner_id, owner_display_name, embedded_dataset_id, llm_id, title, created_at, updated_at
"#;

const GET_SESSION_QUERY: &str = r#"
    SELECT session_id, owner_id, owner_display_name, embedded_dataset_id, llm_id, title, created_at, updated_at
    FROM chat_sessions
    WHERE session_id = $1 AND owner_id = $2
"#;

const GET_SESSIONS_QUERY: &str = r#"
    SELECT session_id, owner_id, owner_display_name, embedded_dataset_id, llm_id, title, created_at, updated_at,
        COUNT(*) OVER() AS total_count
    FROM chat_sessions
    WHERE owner_id = $1
    ORDER BY updated_at DESC
    LIMIT $2 OFFSET $3
"#;

const DELETE_SESSION_QUERY: &str = r#"
    DELETE FROM chat_sessions WHERE session_id = $1 AND owner_id = $2
"#;

const CREATE_MESSAGE_QUERY: &str = r#"
    INSERT INTO chat_messages (session_id, role, content, documents_retrieved, status, created_at)
    VALUES ($1, $2, $3, $4, COALESCE($5, 'complete'), NOW())
    RETURNING message_id, session_id, role, content, documents_retrieved, status, created_at
"#;

const GET_MESSAGES_QUERY: &str = r#"
    SELECT message_id, session_id, role, content, documents_retrieved, status, created_at
    FROM chat_messages
    WHERE session_id = $1
    ORDER BY created_at ASC
"#;

const UPDATE_MESSAGE_CONTENT_STATUS_QUERY: &str = r#"
    UPDATE chat_messages
    SET content = $2, status = $3
    WHERE message_id = $1 AND session_id IN (SELECT session_id FROM chat_sessions WHERE owner_id = $4)
    RETURNING message_id, session_id, role, content, documents_retrieved, status, created_at
"#;

const UPDATE_MESSAGE_STATUS_QUERY: &str = r#"
    UPDATE chat_messages
    SET status = $2
    WHERE message_id = $1 AND session_id IN (SELECT session_id FROM chat_sessions WHERE owner_id = $3)
    RETURNING message_id, session_id, role, content, documents_retrieved, status, created_at
"#;

const GET_MESSAGE_BY_ID_QUERY: &str = r#"
    SELECT message_id, session_id, role, content, documents_retrieved, status, created_at
    FROM chat_messages
    WHERE message_id = $1 AND session_id IN (SELECT session_id FROM chat_sessions WHERE owner_id = $2)
"#;

const GET_LLM_DETAILS_QUERY: &str = r#"
    SELECT name, provider, base_url, config->>'model' as model, api_key_encrypted
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

const GET_BATCH_RETRIEVED_DOCUMENTS_QUERY: &str = r#"
    SELECT
        message_id,
        document_id,
        text,
        similarity_score,
        item_title
    FROM chat_message_retrieved_documents
    WHERE message_id = ANY($1)
    ORDER BY message_id, similarity_score DESC
"#;

#[tracing::instrument(name = "database.create_chat_session", skip(pool), fields(database.system = "postgresql", database.operation = "INSERT", owner_id = %owner_id))]
pub(crate) async fn create_chat_session(
    pool: &Pool<Postgres>,
    owner_id: &str,
    owner_display_name: &str,
    request: &CreateChatSessionRequest,
) -> Result<ChatSession> {
    let session_id = Uuid::new_v4().to_string();

    // Generate default title if not provided
    let title = request.title.clone().unwrap_or_else(|| {
        let now = Utc::now();
        format!("chat-session-{}", now.format("%Y%m%d-%H%M%S"))
    });

    let result = sqlx::query_as::<_, ChatSession>(CREATE_SESSION_QUERY)
        .bind(&session_id)
        .bind(owner_id)
        .bind(owner_display_name)
        .bind(request.embedded_dataset_id)
        .bind(request.llm_id)
        .bind(&title)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.get_chat_session", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id))]
pub(crate) async fn get_chat_session(
    pool: &Pool<Postgres>,
    session_id: &str,
    owner_id: &str,
) -> Result<ChatSession> {
    let result = sqlx::query_as::<_, ChatSession>(GET_SESSION_QUERY)
        .bind(session_id)
        .bind(owner_id)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.get_chat_sessions", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner_id = %owner_id))]
pub(crate) async fn get_chat_sessions(
    pool: &Pool<Postgres>,
    owner_id: &str,
    limit: i64,
    offset: i64,
) -> Result<ChatSessions> {
    let result = sqlx::query_as::<_, ChatSessionWithCount>(GET_SESSIONS_QUERY)
        .bind(owner_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await;

    let (sessions, total_count) = ChatSessionWithCount::into_parts(result?);
    Ok(ChatSessions {
        sessions,
        total_count,
        limit,
        offset,
    })
}

#[tracing::instrument(name = "database.delete_chat_session", skip(pool), fields(database.system = "postgresql", database.operation = "DELETE", owner_id = %owner_id))]
pub(crate) async fn delete_chat_session(
    pool: &Pool<Postgres>,
    session_id: &str,
    owner_id: &str,
) -> Result<()> {
    let result = sqlx::query(DELETE_SESSION_QUERY)
        .bind(session_id)
        .bind(owner_id)
        .execute(pool)
        .await;

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
    status: Option<&str>,
) -> Result<ChatMessage> {
    let result = sqlx::query_as::<_, ChatMessage>(CREATE_MESSAGE_QUERY)
        .bind(session_id)
        .bind(role)
        .bind(content)
        .bind(documents_retrieved)
        .bind(status)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.get_chat_messages", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_chat_messages(
    pool: &Pool<Postgres>,
    session_id: &str,
) -> Result<Vec<ChatMessage>> {
    let result = sqlx::query_as::<_, ChatMessage>(GET_MESSAGES_QUERY)
        .bind(session_id)
        .fetch_all(pool)
        .await;
    match result {
        Ok(messages) => Ok(messages),
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "failed to fetch chat messages");
            Err(e.into())
        }
    }
}

#[tracing::instrument(name = "database.get_llm_details", skip(pool, encryption), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_llm_details(
    pool: &Pool<Postgres>,
    encryption: &EncryptionService,
    llm_id: i32,
) -> Result<(String, String, String, String, Option<String>)> {
    let result = sqlx::query_as::<_, (String, String, String, String, Option<String>)>(
        GET_LLM_DETAILS_QUERY,
    )
    .bind(llm_id)
    .fetch_optional(pool)
    .await;

    let (name, provider, base_url, model, encrypted_api_key) =
        result?.ok_or_else(|| anyhow::anyhow!("LLM not found"))?;

    // Decrypt the API key
    let decrypted_api_key = if let Some(ref encrypted_key) = encrypted_api_key
        && !encrypted_key.is_empty()
    {
        Some(encryption.decrypt(encrypted_key)?)
    } else {
        None
    };

    Ok((name, provider, base_url, model, decrypted_api_key))
}

#[tracing::instrument(name = "database.store_retrieved_documents", skip(pool, documents), fields(database.system = "postgresql", database.operation = "INSERT", count = documents.len()))]
pub(crate) async fn store_retrieved_documents(
    pool: &Pool<Postgres>,
    message_id: i32,
    documents: &[RetrievedDocument],
    batch_size: usize,
) -> Result<()> {
    if documents.is_empty() {
        return Ok(());
    }

    for chunk in documents.chunks(batch_size) {
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

        sqlx::query(BATCH_INSERT_RETRIEVED_DOCUMENTS_QUERY)
            .bind(message_id)
            .bind(&document_ids)
            .bind(&texts)
            .bind(&scores)
            .bind(&item_titles)
            .execute(pool)
            .await?;
    }

    Ok(())
}

#[tracing::instrument(name = "database.get_retrieved_documents", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT"))]
pub(crate) async fn get_retrieved_documents(
    pool: &Pool<Postgres>,
    message_id: i32,
) -> Result<Vec<RetrievedDocument>> {
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

    Ok(documents)
}

/// Batch fetch retrieved documents for multiple messages in a single query (eliminates N+1)
#[tracing::instrument(name = "database.get_batch_retrieved_documents", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", count = message_ids.len()))]
pub(crate) async fn get_batch_retrieved_documents(
    pool: &Pool<Postgres>,
    message_ids: &[i32],
) -> Result<HashMap<i32, Vec<RetrievedDocument>>> {
    if message_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, (i32, Option<String>, String, f32, Option<String>)>(
        GET_BATCH_RETRIEVED_DOCUMENTS_QUERY,
    )
    .bind(message_ids)
    .fetch_all(pool)
    .await?;

    let mut docs_map: HashMap<i32, Vec<RetrievedDocument>> = HashMap::new();

    for (message_id, document_id, text, similarity_score, item_title) in rows {
        docs_map
            .entry(message_id)
            .or_default()
            .push(RetrievedDocument {
                document_id,
                text,
                similarity_score,
                item_title,
            });
    }

    Ok(docs_map)
}

#[tracing::instrument(name = "database.update_message_content_and_status", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", owner = %owner))]
pub(crate) async fn update_message_content_and_status(
    pool: &Pool<Postgres>,
    message_id: i32,
    content: &str,
    status: &str,
    owner: &str,
) -> Result<ChatMessage> {
    let result = sqlx::query_as::<_, ChatMessage>(UPDATE_MESSAGE_CONTENT_STATUS_QUERY)
        .bind(message_id)
        .bind(content)
        .bind(status)
        .bind(owner)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.update_message_status", skip(pool), fields(database.system = "postgresql", database.operation = "UPDATE", owner = %owner))]
pub(crate) async fn update_message_status(
    pool: &Pool<Postgres>,
    message_id: i32,
    status: &str,
    owner: &str,
) -> Result<ChatMessage> {
    let result = sqlx::query_as::<_, ChatMessage>(UPDATE_MESSAGE_STATUS_QUERY)
        .bind(message_id)
        .bind(status)
        .bind(owner)
        .fetch_one(pool)
        .await;

    Ok(result?)
}

#[tracing::instrument(name = "database.get_message_by_id", skip(pool), fields(database.system = "postgresql", database.operation = "SELECT", owner = %owner))]
pub(crate) async fn get_message_by_id(
    pool: &Pool<Postgres>,
    message_id: i32,
    owner: &str,
) -> Result<ChatMessage> {
    let result = sqlx::query_as::<_, ChatMessage>(GET_MESSAGE_BY_ID_QUERY)
        .bind(message_id)
        .bind(owner)
        .fetch_optional(pool)
        .await;

    result?.ok_or_else(|| anyhow::anyhow!("Message not found"))
}
