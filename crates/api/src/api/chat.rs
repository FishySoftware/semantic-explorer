use actix_web::{
    HttpRequest, HttpResponse, Responder, ResponseError, delete, get, post,
    web::{Data, Json, Path, Query},
};
use futures_util::StreamExt;
use qdrant_client::Qdrant;
use serde::Deserialize;
use sqlx::Pool;
use sqlx::Postgres;
use tokio::time::{Duration, interval};

use crate::{
    audit::{ResourceType, events},
    auth::AuthenticatedUser,
    chat::{
        llm,
        models::{
            ChatMessageResponse, ChatMessagesResponse, ChatResponse, ChatSessionResponse,
            ChatSessionsResponse, CreateChatMessageRequest, CreateChatSessionRequest, RAGConfig,
        },
        rag::{self},
    },
    errors::ApiError,
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
#[tracing::instrument(name = "create_chat_session", skip(user, postgres_pool, req))]
pub(crate) async fn create_chat_session(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    request: Json<CreateChatSessionRequest>,
) -> impl Responder {
    match chat::create_chat_session(&postgres_pool.into_inner(), &user, &request).await {
        Ok(session) => {
            events::resource_created_with_request(
                &req,
                &user,
                ResourceType::Session,
                &session.session_id,
            );
            HttpResponse::Created().json(ChatSessionResponse {
                session_id: session.session_id,
                embedded_dataset_id: session.embedded_dataset_id,
                llm_id: session.llm_id,
                title: session.title,
                created_at: session.created_at,
                updated_at: session.updated_at,
            })
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to create chat session");
            ApiError::Internal(format!("error creating chat session: {:?}", e)).error_response()
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
#[tracing::instrument(name = "get_chat_sessions", skip(user, postgres_pool))]
pub(crate) async fn get_chat_sessions(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
) -> impl Responder {
    match chat::get_chat_sessions(&postgres_pool.into_inner(), &user).await {
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
            ApiError::Internal(format!("error fetching sessions: {:?}", e)).error_response()
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
#[tracing::instrument(name = "get_chat_session", skip(user, postgres_pool))]
pub(crate) async fn get_chat_session(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    session_id: Path<String>,
) -> impl Responder {
    match chat::get_chat_session(&postgres_pool.into_inner(), &session_id, &user).await {
        Ok(session) => {
            events::resource_read(&user, ResourceType::Session, &session_id);
            HttpResponse::Ok().json(ChatSessionResponse {
                session_id: session.session_id,
                embedded_dataset_id: session.embedded_dataset_id,
                llm_id: session.llm_id,
                title: session.title,
                created_at: session.created_at,
                updated_at: session.updated_at,
            })
        }
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "failed to fetch chat session");
            ApiError::NotFound(format!("session not found: {:?}", e)).error_response()
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
#[tracing::instrument(name = "delete_chat_session", skip(user, postgres_pool, req))]
pub(crate) async fn delete_chat_session(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    session_id: Path<String>,
) -> impl Responder {
    match chat::delete_chat_session(&postgres_pool.into_inner(), &session_id, &user).await {
        Ok(()) => {
            events::resource_deleted_with_request(&req, &user, ResourceType::Session, &session_id);
            HttpResponse::NoContent().finish()
        }
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "failed to delete chat session");
            ApiError::Internal(format!("error deleting session: {:?}", e)).error_response()
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
#[tracing::instrument(name = "get_chat_messages", skip(user, postgres_pool))]
pub(crate) async fn get_chat_messages(
    user: AuthenticatedUser,
    postgres_pool: Data<Pool<Postgres>>,
    session_id: Path<String>,
) -> impl Responder {
    match chat::get_chat_session(&postgres_pool, &session_id, &user).await {
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
                            status: message.status,
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
                    ApiError::Internal(format!("error fetching messages: {:?}", e)).error_response()
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "session not found");
            ApiError::NotFound(format!("session not found: {:?}", e)).error_response()
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
    skip(user, postgres_pool, request, qdrant_client, req)
)]
pub(crate) async fn send_chat_message(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    session_id: Path<String>,
    request: Json<CreateChatMessageRequest>,
) -> impl Responder {
    // Verify session ownership and get session details
    let session = match chat::get_chat_session(&postgres_pool, &session_id, &user).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "session not found");
            return ApiError::NotFound(format!("session not found: {:?}", e)).error_response();
        }
    };

    // Store user message
    if let Err(e) = chat::add_chat_message(
        &postgres_pool,
        &session_id,
        "user",
        &request.content,
        None,
        Some("complete"),
    )
    .await
    {
        tracing::error!(error = %e, "failed to store user message");
        return ApiError::Internal(format!("error storing message: {:?}", e)).error_response();
    }

    // Track chat message sent
    events::chat_message_sent(&req, &user, &session_id);

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
    let response_content = match llm::generate_response(
        &postgres_pool,
        session.llm_id,
        &request.content,
        &context,
        request.temperature,
        request.max_tokens,
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            tracing::error!(error = %e, "failed to generate LLM response");
            format!("Error generating response: {e}")
        }
    };

    // Transform "Chunk N" references to actual document titles
    let transformed_content =
        rag::replace_chunk_references(&response_content, &retrieved_documents);

    // Store assistant message with transformed content
    let assistant_message = match chat::add_chat_message(
        &postgres_pool,
        &session_id,
        "assistant",
        &transformed_content,
        Some(document_count),
        Some("complete"),
    )
    .await
    {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!(error = %e, "failed to store assistant message");
            return ApiError::Internal(format!("error storing assistant message: {:?}", e))
                .error_response();
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

    // Return response with transformed content and retrieved documents
    let response = ChatResponse {
        message_id: assistant_message.message_id,
        content: transformed_content,
        documents_retrieved: document_count,
        retrieved_documents,
        created_at: assistant_message.created_at,
    };

    HttpResponse::Ok().json(response)
}

#[utoipa::path(
    post,
    path = "/api/chat/sessions/{session_id}/messages/stream",
    tag = "Chat",
    request_body = CreateChatMessageRequest,
    params(
        ("session_id" = String, Path, description = "Chat session ID")
    ),
    responses(
        (status = 200, description = "Server-Sent Events stream", content_type = "text/event-stream"),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Internal Server Error"),
    ),
)]
#[post("/api/chat/sessions/{session_id}/messages/stream")]
#[tracing::instrument(
    name = "stream_chat_message",
    skip(user, postgres_pool, qdrant_client, request, req)
)]
pub(crate) async fn stream_chat_message(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    qdrant_client: Data<Qdrant>,
    session_id: Path<String>,
    request: Json<CreateChatMessageRequest>,
) -> impl Responder {
    use actix_web::http::header;

    // Verify session ownership
    let session = match chat::get_chat_session(&postgres_pool, &session_id, &user).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, session_id = %session_id, "session not found");
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("session not found: {:?}", e)
            }));
        }
    };

    // Store user message
    if let Err(e) = chat::add_chat_message(
        &postgres_pool,
        &session_id,
        "user",
        &request.content,
        None,
        Some("complete"),
    )
    .await
    {
        tracing::error!(error = %e, "failed to store user message");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to store user message: {:?}", e)
        }));
    }

    // Track chat message sent
    events::chat_message_sent(&req, &user, &session_id);

    // Retrieve RAG documents
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
            tracing::warn!(error = %e, "failed to retrieve documents");
            vec![]
        }
    };

    let document_count = retrieved_documents.len() as i32;

    // Create assistant message with status='incomplete'
    let assistant_message = match chat::add_chat_message(
        &postgres_pool,
        &session_id,
        "assistant",
        "",
        Some(document_count),
        Some("incomplete"),
    )
    .await
    {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!(error = %e, "failed to create assistant message");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to create assistant message: {:?}", e)
            }));
        }
    };

    // Store retrieved documents
    if let Err(e) = chat::store_retrieved_documents(
        &postgres_pool,
        assistant_message.message_id,
        &retrieved_documents,
    )
    .await
    {
        tracing::error!(error = %e, "failed to store retrieved documents");
    }

    let message_id = assistant_message.message_id;
    let owner = user.0.clone();
    let postgres_pool_clone = postgres_pool.clone();

    // Create SSE stream
    let stream = async_stream::stream! {
        // Send initial connection event
        yield Ok::<_, actix_web::Error>(
            actix_web::web::Bytes::from("event: connected\ndata: {\"status\":\"connected\"}\n\n")
        );

        // Send retrieval complete event with documents
        let docs_json = match serde_json::to_string(&retrieved_documents) {
            Ok(json) => json,
            Err(e) => {
                yield Err(actix_web::error::ErrorInternalServerError(e));
                return;
            }
        };
        yield Ok(actix_web::web::Bytes::from(format!(
            "event: retrieval_complete\ndata: {{\"message_id\":{},\"documents\":{}}}\n\n",
            message_id, docs_json
        )));

        // Build RAG context
        let context = rag::build_context(&retrieved_documents);

        // Start LLM streaming
        let llm_stream = match llm::generate_response_stream(
            &postgres_pool_clone,
            session.llm_id,
            &request.content,
            &context,
            request.temperature,
            request.max_tokens,
        )
        .await
        {
            Ok(stream) => stream,
            Err(e) => {
                // Update message status to error
                let _ = chat::update_message_status(&postgres_pool_clone, message_id, "error", &owner).await;
                yield Ok(actix_web::web::Bytes::from(format!(
                    "event: error\ndata: {{\"message_id\":{},\"error\":\"{}\"}}\n\n",
                    message_id, e.replace('"', "\\\"")
                )));
                return;
            }
        };

        // Set up heartbeat and timeout
        let mut heartbeat = interval(Duration::from_secs(30));
        heartbeat.tick().await; // Skip first immediate tick

        let mut timeout_timer = interval(Duration::from_secs(300));
        timeout_timer.tick().await; // Skip first immediate tick

        let mut accumulated_content = String::new();
        let mut char_count = 0;
        let start_time = std::time::Instant::now();

        tokio::pin!(llm_stream);

        loop {
            tokio::select! {
                chunk_result = llm_stream.next() => {
                    match chunk_result {
                        Some(Ok(chunk)) => {
                            accumulated_content.push_str(&chunk);
                            char_count += chunk.len();

                            // Send content chunk
                            let chunk_json = serde_json::json!({
                                "message_id": message_id,
                                "content": chunk
                            });
                            yield Ok(actix_web::web::Bytes::from(format!(
                                "event: content\ndata: {}\n\n",
                                chunk_json
                            )));

                            // Send progress update every 100 characters
                            if char_count % 100 == 0 {
                                let progress_json = serde_json::json!({
                                    "message_id": message_id,
                                    "char_count": char_count,
                                    "elapsed_seconds": start_time.elapsed().as_secs()
                                });
                                yield Ok(actix_web::web::Bytes::from(format!(
                                    "event: progress\ndata: {}\n\n",
                                    progress_json
                                )));
                            }
                        }
                        Some(Err(e)) => {
                            let _ = chat::update_message_status(&postgres_pool_clone, message_id, "error", &owner).await;
                            yield Ok(actix_web::web::Bytes::from(format!(
                                "event: error\ndata: {{\"message_id\":{},\"error\":\"{}\"}}\n\n",
                                message_id, e.replace('"', "\\\"")
                            )));
                            return;
                        }
                        None => {
                            // Stream complete - transform and save
                            let transformed_content = rag::replace_chunk_references(
                                &accumulated_content,
                                &retrieved_documents
                            );

                            if let Err(e) = chat::update_message_content_and_status(
                                &postgres_pool_clone,
                                message_id,
                                &transformed_content,
                                "complete",
                                &owner,
                            )
                            .await
                            {
                                tracing::error!(error = %e, "failed to update message");
                                yield Ok(actix_web::web::Bytes::from(format!(
                                    "event: error\ndata: {{\"message_id\":{},\"error\":\"failed to save message\"}}\n\n",
                                    message_id
                                )));
                                return;
                            }

                            // Send the transformed content with the complete event
                            let complete_json = serde_json::json!({
                                "message_id": message_id,
                                "content": transformed_content
                            });
                            yield Ok(actix_web::web::Bytes::from(format!(
                                "event: complete\ndata: {}\n\n",
                                complete_json
                            )));
                            return;
                        }
                    }
                }
                _ = heartbeat.tick() => {
                    yield Ok(actix_web::web::Bytes::from(": heartbeat\n\n"));
                }
                _ = timeout_timer.tick() => {
                    // Timeout reached
                    let _ = chat::update_message_status(&postgres_pool_clone, message_id, "error", &owner).await;
                    yield Ok(actix_web::web::Bytes::from(format!(
                        "event: timeout\ndata: {{\"message_id\":{}}}\n\n",
                        message_id
                    )));
                    return;
                }
            }
        }
    };

    HttpResponse::Ok()
        .insert_header(header::ContentType(mime::TEXT_EVENT_STREAM))
        .insert_header(header::CacheControl(vec![header::CacheDirective::NoCache]))
        .insert_header(("X-Accel-Buffering", "no"))
        .streaming(stream)
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegenerateMessageQuery {
    #[serde(default)]
    pub stream: bool,
}

#[utoipa::path(
    post,
    path = "/api/chat/messages/{message_id}/regenerate",
    tag = "Chat",
    params(
        ("message_id" = i32, Path, description = "Message ID to regenerate"),
        ("stream" = Option<bool>, Query, description = "Use streaming response")
    ),
    responses(
        (status = 200, description = "OK", body = ChatResponse),
        (status = 404, description = "Message not found"),
        (status = 500, description = "Internal Server Error"),
    ),
)]
#[post("/api/chat/messages/{message_id}/regenerate")]
#[tracing::instrument(name = "regenerate_chat_message", skip(user, postgres_pool, req))]
pub(crate) async fn regenerate_chat_message(
    user: AuthenticatedUser,
    req: HttpRequest,
    postgres_pool: Data<Pool<Postgres>>,
    message_id: Path<i32>,
    query: Query<RegenerateMessageQuery>,
) -> impl Responder {
    let message_id = message_id.into_inner();

    // Get the message and verify ownership
    let message = match chat::get_message_by_id(&postgres_pool, message_id, &user.0).await {
        Ok(msg) => msg,
        Err(e) => {
            tracing::error!(error = %e, message_id, "message not found");
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("message not found: {:?}", e)
            }));
        }
    };

    // Verify it's an assistant message
    if message.role != "assistant" {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "can only regenerate assistant messages"
        }));
    }

    // Get session info
    let session = match chat::get_chat_session(&postgres_pool, &message.session_id, &user).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, session_id = %message.session_id, "session not found");
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": format!("session not found: {:?}", e)
            }));
        }
    };

    // Get existing retrieved documents (reuse them)
    let retrieved_documents = match chat::get_retrieved_documents(&postgres_pool, message_id).await
    {
        Ok(docs) => docs,
        Err(e) => {
            tracing::error!(error = %e, message_id, "failed to get retrieved documents");
            vec![]
        }
    };

    // Get previous user message to regenerate from
    let messages = match chat::get_chat_messages(&postgres_pool, &message.session_id).await {
        Ok(msgs) => msgs,
        Err(e) => {
            tracing::error!(error = %e, "failed to get messages");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to get messages: {:?}", e)
            }));
        }
    };

    // Find the user message before this assistant message
    let user_message = messages
        .iter()
        .take_while(|m| m.message_id != message_id)
        .filter(|m| m.role == "user")
        .last();

    let user_query = match user_message {
        Some(msg) => &msg.content,
        None => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "error": "no user message found before assistant message"
            }));
        }
    };

    // Build context from retrieved documents
    let context = rag::build_context(&retrieved_documents);

    // Check if streaming is requested
    if query.stream {
        return HttpResponse::NotImplemented().json(serde_json::json!({
            "error": "streaming regeneration not yet implemented - use non-streaming for now"
        }));
    }

    // Generate new response (non-streaming)
    let response_content = match llm::generate_response(
        &postgres_pool,
        session.llm_id,
        user_query,
        &context,
        None,
        None,
    )
    .await
    {
        Ok(response) => response,
        Err(e) => {
            tracing::error!(error = %e, "failed to generate LLM response");
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("failed to generate response: {}", e)
            }));
        }
    };

    // Transform content
    let transformed_content =
        rag::replace_chunk_references(&response_content, &retrieved_documents);

    // Update message
    if let Err(e) = chat::update_message_content_and_status(
        &postgres_pool,
        message_id,
        &transformed_content,
        "complete",
        &user.0,
    )
    .await
    {
        tracing::error!(error = %e, "failed to update message");
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("failed to update message: {:?}", e)
        }));
    }

    // Track regeneration event
    events::chat_message_sent(&req, &user, &message.session_id);

    // Return updated message
    let response = ChatResponse {
        message_id,
        content: transformed_content,
        documents_retrieved: retrieved_documents.len() as i32,
        retrieved_documents,
        created_at: message.created_at,
    };

    HttpResponse::Ok().json(response)
}
