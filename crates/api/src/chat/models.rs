use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use sqlx::types::chrono::{self, DateTime, Utc};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ChatSessionResponse {
    pub session_id: String,
    pub embedded_dataset_id: i32,
    pub llm_id: i32,
    pub title: String,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: chrono::DateTime<Utc>,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ChatMessageResponse {
    pub message_id: i32,
    pub role: String,
    pub content: String,
    pub documents_retrieved: Option<i32>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ChatSessionsResponse {
    pub sessions: Vec<ChatSessionResponse>,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct ChatMessagesResponse {
    pub messages: Vec<ChatMessageResponse>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, FromRow)]
pub(crate) struct ChatSession {
    pub session_id: String,
    pub owner: String,
    pub embedded_dataset_id: i32,
    pub llm_id: i32,
    pub title: String,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
    #[schema(value_type = String, format = DateTime)]
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug, FromRow)]
pub(crate) struct ChatMessage {
    pub message_id: i32,
    pub session_id: String,
    pub role: String, // "user" or "assistant"
    pub content: String,
    pub documents_retrieved: Option<i32>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub(crate) struct CreateChatMessageRequest {
    pub content: String,
    #[serde(default)]
    pub max_context_documents: Option<i32>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub(crate) struct ChatResponse {
    pub message_id: i32,
    pub content: String,
    pub documents_retrieved: i32,
    pub retrieved_documents: Vec<RetrievedDocument>,
    #[schema(value_type = String, format = DateTime)]
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug, Clone)]
pub(crate) struct RetrievedDocument {
    pub document_id: Option<String>,
    pub text: String,
    pub similarity_score: f32,
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize, ToSchema, Debug)]
pub(crate) struct CreateChatSessionRequest {
    pub embedded_dataset_id: i32,
    pub llm_id: i32,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RAGConfig {
    pub max_context_documents: usize,
    pub min_similarity_score: f32,
    pub max_tokens_context: usize,
}

impl Default for RAGConfig {
    fn default() -> Self {
        Self {
            max_context_documents: 5,
            min_similarity_score: 0.5,
            max_tokens_context: 3000,
        }
    }
}
