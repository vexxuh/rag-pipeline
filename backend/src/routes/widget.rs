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
use crate::middleware::embed_auth::EmbedContext;
use crate::services::{audit, llm_provider};
use crate::state::AppState;

#[derive(Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct WidgetConfigResponse {
    pub widget_title: String,
    pub primary_color: String,
    pub greeting_message: String,
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/widget/config", tag = "Widget", security(("embed_key" = [])), responses((status = 200, body = WidgetConfigResponse))))]
pub async fn get_config(
    State(state): State<AppState>,
    ctx: EmbedContext,
) -> Result<Json<WidgetConfigResponse>, AppError> {
    if !state.config.features.widget_enabled {
        return Err(AppError::FeatureDisabled("Widget".to_string()));
    }

    Ok(Json(WidgetConfigResponse {
        widget_title: ctx.embed_key.widget_title,
        primary_color: ctx.embed_key.primary_color,
        greeting_message: ctx.embed_key.greeting_message,
    }))
}

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreateWidgetConversationRequest {
    pub title: Option<String>,
}

#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/widget/conversations", tag = "Widget", security(("embed_key" = [])), request_body = CreateWidgetConversationRequest, responses((status = 200, body = Conversation))))]
pub async fn create_conversation(
    State(state): State<AppState>,
    ctx: EmbedContext,
    Json(payload): Json<CreateWidgetConversationRequest>,
) -> Result<Json<Conversation>, AppError> {
    if !state.config.features.widget_enabled {
        return Err(AppError::FeatureDisabled("Widget".to_string()));
    }

    // Ensure session exists for rate limiting
    state
        .widget_session_repo
        .get_or_create(&ctx.embed_key.id, &ctx.session_id)
        .await?;

    let title = payload
        .title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Widget Chat".to_string());

    let conv = state
        .conversation_repo
        .create_widget(&ctx.embed_key.id, &ctx.session_id, &title)
        .await?;

    // Increment conversation stats (fire-and-forget)
    let repo = state.embed_key_repo.clone();
    let key_id = ctx.embed_key.id.clone();
    tokio::spawn(async move {
        let _ = repo.increment_stats(&key_id, 1, 0).await;
    });

    Ok(Json(conv))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/widget/conversations", tag = "Widget", security(("embed_key" = [])), responses((status = 200, body = Vec<Conversation>))))]
pub async fn list_conversations(
    State(state): State<AppState>,
    ctx: EmbedContext,
) -> Result<Json<Vec<Conversation>>, AppError> {
    if !state.config.features.widget_enabled {
        return Err(AppError::FeatureDisabled("Widget".to_string()));
    }

    let convs = state
        .conversation_repo
        .list_by_session(&ctx.session_id, &ctx.embed_key.id)
        .await?;

    Ok(Json(convs))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/widget/conversations/{id}/messages", tag = "Widget", security(("embed_key" = [])), params(("id" = String, Path, description = "Conversation ID")), responses((status = 200, body = Vec<Message>))))]
pub async fn get_messages(
    State(state): State<AppState>,
    ctx: EmbedContext,
    Path(conversation_id): Path<String>,
) -> Result<Json<Vec<Message>>, AppError> {
    if !state.config.features.widget_enabled {
        return Err(AppError::FeatureDisabled("Widget".to_string()));
    }

    // Verify conversation belongs to this session + embed key
    state
        .conversation_repo
        .get_widget(&conversation_id, &ctx.session_id, &ctx.embed_key.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    let messages = state
        .conversation_repo
        .get_messages(&conversation_id)
        .await?;

    Ok(Json(messages))
}

#[derive(Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct WidgetSendMessageRequest {
    pub message: String,
}

#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/widget/conversations/{id}/messages", tag = "Widget", security(("embed_key" = [])), params(("id" = String, Path, description = "Conversation ID")), request_body = WidgetSendMessageRequest, responses((status = 200, description = "SSE stream of assistant response"))))]
pub async fn send_message(
    State(state): State<AppState>,
    ctx: EmbedContext,
    Path(conversation_id): Path<String>,
    Json(payload): Json<WidgetSendMessageRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    if !state.config.features.widget_enabled {
        return Err(AppError::FeatureDisabled("Widget".to_string()));
    }

    if payload.message.trim().is_empty() {
        return Err(AppError::Validation("Message cannot be empty".to_string()));
    }

    // Verify conversation belongs to this session + embed key
    state
        .conversation_repo
        .get_widget(&conversation_id, &ctx.session_id, &ctx.embed_key.id)
        .await?
        .ok_or_else(|| AppError::NotFound("Conversation not found".to_string()))?;

    // Rate limit check
    let msg_count = state
        .widget_session_repo
        .increment_message_count(&ctx.embed_key.id, &ctx.session_id)
        .await?;

    if msg_count > ctx.embed_key.rate_limit {
        return Err(AppError::RateLimited);
    }

    // Persist user message
    state
        .conversation_repo
        .add_message(&conversation_id, "user", &payload.message)
        .await?;

    // Resolve provider/model from embed key config or system defaults
    let provider_name = if ctx.embed_key.provider.is_empty() {
        state.config.llm.default_provider.clone()
    } else {
        ctx.embed_key.provider.clone()
    };

    let model_name = if ctx.embed_key.model.is_empty() {
        state.config.llm.default_model.clone()
    } else {
        ctx.embed_key.model.clone()
    };

    let api_key = if ctx.embed_key.api_key_encrypted.is_empty() {
        return Err(AppError::Validation(
            "Widget has no API key configured. Contact the administrator.".to_string(),
        ));
    } else {
        ctx.embed_key.api_key_encrypted.clone()
    };

    let system_prompt = if ctx.embed_key.system_prompt.is_empty() {
        state.config.llm.default_system_prompt.clone()
    } else {
        ctx.embed_key.system_prompt.clone()
    };

    // RAG context retrieval
    let mut rag_context = String::new();

    let embedding_model_name = state.config.llm.default_embedding_model.clone();

    if let Ok(emb_client) = llm_provider::create_embeddings_client(&provider_name, &api_key) {
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
                        tracing::warn!("Widget RAG search failed: {e}");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Widget failed to embed query for RAG: {e}");
            }
        }
    }

    // Build final system prompt
    let final_system_prompt = format!("{system_prompt}{rag_context}");

    // Create completion client
    let completion_client = llm_provider::create_completion_client(&provider_name, &api_key)
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

    // Fire-and-forget: audit log + stats
    let audit_repo = state.audit_log_repo.clone();
    let embed_key_repo = state.embed_key_repo.clone();
    let key_id = ctx.embed_key.id.clone();
    let conv_id = conversation_id.clone();
    tokio::spawn(async move {
        audit::log(
            &audit_repo,
            None,
            "widget.message",
            Some("conversation"),
            Some(&conv_id),
            "Widget chat message",
            None,
            None,
        );
        let _ = embed_key_repo.increment_stats(&key_id, 0, 2).await;
    });

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
