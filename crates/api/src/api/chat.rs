use actix_web::{
    HttpResponse, Responder, delete, get, post,
    web::{Data, Json, Path},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::Qdrant;
use sqlx::types::chrono::Utc;
use sqlx::{Pool, Postgres};

use crate::{
    auth::extract_username,
    chat::{
        models::{
            ChatMessageResponse, ChatMessagesResponse, ChatResponse, ChatSessionResponse,
            ChatSessionsResponse, CreateChatMessageRequest, CreateChatSessionRequest, RAGConfig,
        },
        rag::{self},
    },
    storage::postgres::chat,
};

#[utoipa::path(
    responses(
        (status = 201, description = "Created", body = String),
        (status = 500, description = "Internal Server Error"),
    ),
    request_body = CreateChatSessionRequest,
    tag = "Chat",
)]
#[post("/api/chat/sessions")]
#[tracing::instrument(name = "create_chat_session", skip(auth, postgres_pool))]
pub(crate) async fn create_chat_session(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    request: Json<CreateChatSessionRequest>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match chat::create_chat_session(&postgres_pool.into_inner(), &username, &request).await {
        Ok(session) => HttpResponse::Created().json(ChatSessionResponse {
            session_id: session.session_id,
            embedded_dataset_id: session.embedded_dataset_id,
            llm_id: session.llm_id,
            title: session.title,
            created_at: session.created_at,
            updated_at: session.updated_at,
        }),
        Err(e) => {
            tracing::error!(error = %e, "failed to create chat session");
            HttpResponse::InternalServerError().body(format!("error creating chat session: {e:?}"))
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = ChatSessionsResponse),
        (status = 500, description = "Internal Server Error"),
    ),
    tag = "Chat",
)]
#[get("/api/chat/sessions")]
#[tracing::instrument(name = "get_chat_sessions", skip(auth, postgres_pool))]
pub(crate) async fn get_chat_sessions(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match chat::get_chat_sessions(&postgres_pool.into_inner(), &username).await {
        Ok(sessions) => {
            let sessions_response: Vec<ChatSessionResponse> = sessions
                .into_iter()
                .map(|s| ChatSessionResponse {
                    session_id: s.session_id,
                    embedded_dataset_id: s.embedded_dataset_id,
                    llm_id: s.llm_id,
                    title: s.title,
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                })
                .collect();
            HttpResponse::Ok().json(ChatSessionsResponse {
                sessions: sessions_response,
            })
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to fetch chat sessions");
            HttpResponse::InternalServerError().body(format!("error fetching sessions: {e:?}"))
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = ChatSessionResponse),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("session_id" = String, Path, description = "Chat session ID")
    ),
    tag = "Chat",
)]
#[get("/api/chat/sessions/{session_id}")]
#[tracing::instrument(name = "get_chat_session", skip(auth, postgres_pool))]
pub(crate) async fn get_chat_session(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    session_id: Path<String>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match chat::get_chat_session(&postgres_pool.into_inner(), &session_id, &username).await {
        Ok(session) => HttpResponse::Ok().json(ChatSessionResponse {
            session_id: session.session_id,
            embedded_dataset_id: session.embedded_dataset_id,
            llm_id: session.llm_id,
            title: session.title,
            created_at: session.created_at,
            updated_at: session.updated_at,
        }),
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "failed to fetch chat session");
            HttpResponse::NotFound().body(format!("session not found: {e:?}"))
        }
    }
}

#[utoipa::path(
    responses(
        (status = 204, description = "No Content"),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("session_id" = String, Path, description = "Chat session ID")
    ),
    tag = "Chat",
)]
#[delete("/api/chat/sessions/{session_id}")]
#[tracing::instrument(name = "delete_chat_session", skip(auth, postgres_pool))]
pub(crate) async fn delete_chat_session(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    session_id: Path<String>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match chat::delete_chat_session(&postgres_pool.into_inner(), &session_id, &username).await {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "failed to delete chat session");
            HttpResponse::InternalServerError().body(format!("error deleting session: {e:?}"))
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = ChatMessagesResponse),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    params(
        ("session_id" = String, Path, description = "Chat session ID")
    ),
    tag = "Chat",
)]
#[get("/api/chat/sessions/{session_id}/messages")]
#[tracing::instrument(name = "get_chat_messages", skip(auth, postgres_pool))]
pub(crate) async fn get_chat_messages(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    session_id: Path<String>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    match chat::get_chat_session(&postgres_pool, &session_id, &username).await {
        Ok(_) => {
            // Session exists and user owns it
            match chat::get_chat_messages(&postgres_pool, &session_id).await {
                Ok(messages) => {
                    let messages_response: Vec<ChatMessageResponse> = messages
                        .into_iter()
                        .map(|m| ChatMessageResponse {
                            message_id: m.message_id,
                            role: m.role,
                            content: m.content,
                            documents_retrieved: m.documents_retrieved,
                            created_at: m.created_at,
                        })
                        .collect();
                    HttpResponse::Ok().json(ChatMessagesResponse {
                        messages: messages_response,
                    })
                }
                Err(e) => {
                    tracing::error!(error = %e, session_id = %session_id, "failed to fetch messages");
                    HttpResponse::InternalServerError()
                        .body(format!("error fetching messages: {e:?}"))
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "session not found");
            HttpResponse::NotFound().body(format!("session not found: {e:?}"))
        }
    }
}

#[utoipa::path(
    responses(
        (status = 200, description = "OK", body = ChatResponse),
        (status = 404, description = "Not Found"),
        (status = 500, description = "Internal Server Error"),
    ),
    request_body = CreateChatMessageRequest,
    params(
        ("session_id" = String, Path, description = "Chat session ID")
    ),
    tag = "Chat",
)]
#[post("/api/chat/sessions/{session_id}/messages")]
#[tracing::instrument(
    name = "send_chat_message",
    skip(auth, postgres_pool, request, qdrant_client)
)]
pub(crate) async fn send_chat_message(
    auth: Authenticated,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    session_id: Path<String>,
    request: Json<CreateChatMessageRequest>,
) -> impl Responder {
    let username = match extract_username(&auth) {
        Ok(username) => username,
        Err(e) => return e,
    };

    // Verify session ownership and get session details
    let session = match chat::get_chat_session(&postgres_pool, &session_id, &username).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "session not found");
            return HttpResponse::NotFound().body(format!("session not found: {e:?}"));
        }
    };

    // Store user message
    if let Err(e) =
        chat::add_chat_message(&postgres_pool, &session_id, "user", &request.content, None).await
    {
        tracing::error!(error = %e, "failed to store user message");
        return HttpResponse::InternalServerError().body(format!("error storing message: {e:?}"));
    }

    // Retrieve relevant documents using RAG
    let rag_config = RAGConfig::default();
    let retrieved_documents = match rag::retrieve_documents(
        &postgres_pool,
        qdrant_client.as_ref(),
        session.embedded_dataset_id,
        &request.content,
        &rag_config,
    )
    .await
    {
        Ok(docs) => docs,
        Err(e) => {
            tracing::warn!(error = %e, "failed to retrieve documents for RAG");
            vec![]
        }
    };

    let document_count = retrieved_documents.len() as i32;

    // Build context from retrieved documents
    let context = rag::build_context(&retrieved_documents);

    // Generate response using LLM with RAG context
    let response_content =
        match generate_llm_response(&postgres_pool, session.llm_id, &request.content, &context)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!(error = %e, "failed to generate LLM response");
                format!("Error generating response: {e}")
            }
        };

    // Store assistant message
    if let Err(e) = chat::add_chat_message(
        &postgres_pool,
        &session_id,
        "assistant",
        &response_content,
        Some(document_count),
    )
    .await
    {
        tracing::error!(error = %e, "failed to store assistant message");
    }

    // Return response with retrieved documents
    let response = ChatResponse {
        message_id: 0,
        content: response_content,
        documents_retrieved: document_count,
        retrieved_documents,
        created_at: Utc::now(),
    };

    HttpResponse::Ok().json(response)
}

/// Generate an LLM response with RAG context
#[tracing::instrument(name = "generate_llm_response", skip(postgres_pool, query, context))]
async fn generate_llm_response(
    postgres_pool: &Pool<Postgres>,
    llm_id: i32,
    query: &str,
    context: &str,
) -> Result<String, String> {
    // Fetch LLM details from database using dynamic query
    let row = sqlx::query_as::<_, (String, String, String, String, String)>(
        r#"
        SELECT name, provider, api_base, model, api_key_env
        FROM llms
        WHERE llm_id = $1
        "#,
    )
    .bind(llm_id)
    .fetch_optional(postgres_pool)
    .await
    .map_err(|e| format!("database error: {e}"))?
    .ok_or_else(|| "LLM not found".to_string())?;

    let (name, provider, api_base, model, api_key_env) = row;

    // Build the prompt with RAG context
    const SYSTEM_PROMPT: &str = "You are a helpful assistant that answers questions based on the provided context. Always cite the source of your information when possible.";

    let user_prompt = format!(
        "{}\n\nQuestion: {}\n\nPlease provide a helpful answer based on the context above.",
        context, query
    );

    // Call the appropriate LLM API
    let response_text = match provider.to_lowercase().as_str() {
        "openai" => {
            call_openai_api(&api_base, &api_key_env, &model, SYSTEM_PROMPT, &user_prompt).await?
        }
        "cohere" => call_cohere_api(&api_base, &api_key_env, &model, &user_prompt).await?,
        _ => {
            return Err(format!("unsupported LLM provider: {}", provider));
        }
    };

    tracing::debug!(llm_name = %name, "generated LLM response");
    Ok(response_text)
}

/// Call OpenAI API for chat completion
async fn call_openai_api(
    api_base: &str,
    api_key_env: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
) -> Result<String, String> {
    // Get API key from environment
    let api_key = std::env::var(api_key_env)
        .map_err(|_| format!("API key environment variable not set: {}", api_key_env))?;

    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ],
        "temperature": 0.7,
        "max_tokens": 2000,
    });

    let response = client
        .post(format!("{}/v1/chat/completions", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {e}"))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error: {}", error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse OpenAI response: {e}"))?;

    let response_text = response_json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "unexpected OpenAI response format".to_string())?
        .to_string();

    Ok(response_text)
}

/// Call Cohere API for text generation
async fn call_cohere_api(
    api_base: &str,
    api_key_env: &str,
    model: &str,
    prompt: &str,
) -> Result<String, String> {
    // Get API key from environment
    let api_key = std::env::var(api_key_env)
        .map_err(|_| format!("API key environment variable not set: {}", api_key_env))?;

    let client = reqwest::Client::new();

    let request_body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "max_tokens": 2000,
        "temperature": 0.7,
    });

    let response = client
        .post(format!("{}/generate", api_base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await
        .map_err(|e| format!("Cohere API request failed: {e}"))?;

    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Cohere API error: {}", error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("failed to parse Cohere response: {e}"))?;

    let response_text = response_json
        .get("generations")
        .and_then(|g| g.get(0))
        .and_then(|g| g.get("text"))
        .and_then(|t| t.as_str())
        .ok_or_else(|| "unexpected Cohere response format".to_string())?
        .to_string();

    Ok(response_text)
}
