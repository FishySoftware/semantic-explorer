use actix_web::{
    HttpResponse, Responder, delete, get, post,
    web::{Data, Json, Path},
};
use actix_web_openidconnect::openid_middleware::Authenticated;
use qdrant_client::Qdrant;
use sqlx::Pool;
use sqlx::Postgres;

use crate::{
    auth::extract_username,
    chat::{
        llm,
        models::{
            ChatMessageResponse, ChatMessagesResponse, ChatResponse, ChatSessionResponse,
            ChatSessionsResponse, CreateChatMessageRequest, CreateChatSessionRequest, RAGConfig,
        },
        rag::{self},
    },
    errors::not_found,
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
            not_found(format!("session not found: {e:?}"))
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
                    let mut messages_response: Vec<ChatMessageResponse> = Vec::new();

                    for message in messages {
                        // Fetch retrieved documents for assistant messages
                        let retrieved_documents = if message.role == "assistant" {
                            match chat::get_retrieved_documents(&postgres_pool, message.message_id)
                                .await
                            {
                                Ok(docs) => {
                                    if docs.is_empty() {
                                        None
                                    } else {
                                        Some(docs)
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, message_id = message.message_id, "failed to fetch retrieved documents");
                                    None
                                }
                            }
                        } else {
                            None
                        };

                        messages_response.push(ChatMessageResponse {
                            message_id: message.message_id,
                            role: message.role,
                            content: message.content,
                            documents_retrieved: message.documents_retrieved,
                            retrieved_documents,
                            created_at: message.created_at,
                        });
                    }

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
            not_found(format!("session not found: {e:?}"))
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
            return not_found(format!("session not found: {e:?}"));
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
    let mut rag_config = RAGConfig::default();
    if let Some(max_docs) = request.max_context_documents {
        rag_config.max_context_documents = max_docs.max(1) as usize;
    }
    if let Some(min_score) = request.min_similarity_score {
        rag_config.min_similarity_score = min_score.clamp(0.0, 1.0);
    }

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
        match llm::generate_response(&postgres_pool, session.llm_id, &request.content, &context)
            .await
        {
            Ok(response) => response,
            Err(e) => {
                tracing::error!(error = %e, "failed to generate LLM response");
                format!("Error generating response: {e}")
            }
        };

    // Store assistant message
    let assistant_message = match chat::add_chat_message(
        &postgres_pool,
        &session_id,
        "assistant",
        &response_content,
        Some(document_count),
    )
    .await
    {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!(error = %e, "failed to store assistant message");
            return HttpResponse::InternalServerError()
                .body(format!("error storing assistant message: {e:?}"));
        }
    };

    // Store retrieved documents for this message
    if let Err(e) = chat::store_retrieved_documents(
        &postgres_pool,
        assistant_message.message_id,
        &retrieved_documents,
    )
    .await
    {
        tracing::error!(error = %e, "failed to store retrieved documents");
        // Continue anyway - this is not critical
    }

    // Return response with retrieved documents
    let response = ChatResponse {
        message_id: assistant_message.message_id,
        content: response_content,
        documents_retrieved: document_count,
        retrieved_documents,
        created_at: assistant_message.created_at,
    };

    HttpResponse::Ok().json(response)
}
