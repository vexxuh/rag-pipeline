use axum::{
    extract::{Path, State},
    response::sse::{Event, Sse},
    Json,
};
use futures::stream::Stream;
use rig::completion::Prompt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio_stream::StreamExt;

use crate::db::models::conversation::{Conversation, Message};
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::services::{audit, llm_provider};
use crate::state::AppState;

// ── Conversations CRUD ──────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateConversationRequest {
    pub title: Option<String>,
}

pub async fn create_conversation(
    State(state): State<AppState>,
    claims: Claims,
    Json(payload): Json<CreateConversationRequest>,
) -> Result<Json<Conversation>, AppError> {
    let title = payload
        .title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "New Chat".to_string());

    let conv = state.conversation_repo.create(&claims.sub, &title).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "chat.create",
        Some("conversation"),
        Some(&conv.id),
        &format!("Created conversation '{}'", conv.title),
        None,
        None,
    );

    Ok(Json(conv))
}

pub async fn list_conversations(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<Conversation>>, AppError> {
    let convs = state.conversation_repo.list_by_user(&claims.sub).await?;
    Ok(Json(convs))
}

#[derive(Serialize)]
pub struct ConversationWithMessages {
    #[serde(flatten)]
    pub conversation: Conversation,
    pub messages: Vec<Message>,
}

pub async fn get_conversation(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<ConversationWithMessages>, AppError> {
    let conv = state
        .conversation_repo
        .get(&id, &claims.sub)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    let messages = state.conversation_repo.get_messages(&id).await?;

    Ok(Json(ConversationWithMessages {
        conversation: conv,
        messages,
    }))
}

pub async fn delete_conversation(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    state.conversation_repo.soft_delete(&id, &claims.sub).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "chat.delete",
        Some("conversation"),
        Some(&id),
        "Deleted conversation",
        None,
        None,
    );

    Ok(())
}

// ── Send Message (with LLM + persistence) ───────────────────

#[derive(Deserialize)]
pub struct SendMessageRequest {
    pub message: String,
}

pub async fn send_message(
    State(state): State<AppState>,
    claims: Claims,
    Path(conversation_id): Path<String>,
    Json(payload): Json<SendMessageRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if payload.message.trim().is_empty() {
        return Err(AppError::Validation("Message cannot be empty".to_string()));
    }

    // Verify conversation belongs to user
    let conv = state
        .conversation_repo
        .get(&conversation_id, &claims.sub)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    // Persist user message
    state
        .conversation_repo
        .add_message(&conversation_id, "user", &payload.message)
        .await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "chat.message",
        Some("conversation"),
        Some(&conversation_id),
        "Sent chat message",
        None,
        None,
    );

    // Auto-title on first message
    if conv.title == "New Chat" {
        let title: String = payload.message.chars().take(50).collect();
        let _ = state
            .conversation_repo
            .update_title(&conversation_id, &title)
            .await;
    }

    // Resolve provider/model from user preferences
    let prefs = state.settings_repo.get_preferences(&claims.sub).await?;

    let provider_name = prefs
        .as_ref()
        .map(|p| p.preferred_provider.clone())
        .unwrap_or_else(|| state.config.llm.default_provider.clone());

    let model_name = prefs
        .as_ref()
        .map(|p| p.preferred_model.clone())
        .unwrap_or_else(|| state.config.llm.default_model.clone());

    let system_prompt = prefs
        .as_ref()
        .map(|p| p.system_prompt.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| state.config.llm.default_system_prompt.clone());

    // Get API key
    let api_key = state
        .settings_repo
        .get_api_key(&claims.sub, &provider_name)
        .await?
        .ok_or_else(|| {
            AppError::Validation(format!(
                "No API key configured for provider '{provider_name}'. Add one in Settings."
            ))
        })?;

    // RAG context retrieval: embed the user's message and search for relevant chunks
    let mut rag_context = String::new();

    let embedding_provider = prefs
        .as_ref()
        .map(|p| p.preferred_provider.clone())
        .unwrap_or_else(|| state.config.llm.default_provider.clone());
    let embedding_model_name = prefs
        .as_ref()
        .map(|p| p.preferred_embedding_model.clone())
        .unwrap_or_else(|| state.config.llm.default_embedding_model.clone());

    let embedding_api_key = state
        .settings_repo
        .get_api_key(&claims.sub, &embedding_provider)
        .await
        .ok()
        .flatten();

    if let Some(ref emb_key) = embedding_api_key {
        if let Ok(emb_client) =
            llm_provider::create_embeddings_client(&embedding_provider, emb_key)
        {
            let emb_model = rig::client::embeddings::EmbeddingsClientDyn::embedding_model(
                emb_client.as_ref(),
                &embedding_model_name,
            );

            match emb_model.embed_text(&payload.message).await {
                Ok(query_embedding) => {
                    match state.vector_service.search(query_embedding.vec, 5).await {
                        Ok(results) if !results.is_empty() => {
                            let context_parts: Vec<String> = results
                                .iter()
                                .filter(|r| !r.content.is_empty())
                                .map(|r| r.content.clone())
                                .collect();

                            if !context_parts.is_empty() {
                                rag_context = format!(
                                    "\n\nUse the following context from the knowledge base to help answer the user's question. If the context is not relevant, you may ignore it.\n\n---\n{}\n---\n",
                                    context_parts.join("\n\n")
                                );
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!("RAG search failed: {e}");
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to embed query for RAG: {e}");
                }
            }
        }
    }

    // Build final system prompt with RAG context
    let final_system_prompt = format!("{system_prompt}{rag_context}");

    // Create completion client via rig
    let completion_client =
        llm_provider::create_completion_client(&provider_name, &api_key)
            .map_err(AppError::Internal)?;

    let agent = completion_client
        .agent(&model_name)
        .preamble(&final_system_prompt)
        .build();

    let message = payload.message.clone();

    // Get LLM response
    let response = agent
        .prompt(&message)
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("LLM error: {e}")))?;

    // Persist assistant message
    state
        .conversation_repo
        .add_message(&conversation_id, "assistant", &response)
        .await?;

    // Update conversation timestamp
    let _ = state.conversation_repo.touch(&conversation_id).await;

    // Stream response as SSE
    let words: Vec<String> = response
        .split_inclusive(' ')
        .map(|s| s.to_string())
        .collect();

    let stream = tokio_stream::iter(words)
        .throttle(std::time::Duration::from_millis(20))
        .map(|word| Ok(Event::default().data(word)))
        .chain(tokio_stream::once(Ok(Event::default().data("[DONE]"))));

    Ok(Sse::new(stream))
}
