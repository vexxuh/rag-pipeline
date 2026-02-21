use axum::{
    extract::{Multipart, Path, State},
    Json,
};
use std::sync::Arc;

use crate::db::models::document::DocumentStatus;
use crate::db::models::document_chunk::DocumentChunkRepository;
use crate::dto::document::DocumentResponse;
use crate::errors::AppError;
use crate::middleware::auth::{require_admin, require_maintainer, Claims};
use crate::services::audit;
use crate::services::storage::StorageService;
use crate::services::vector::VectorService;
use crate::state::AppState;

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/documents/limits", tag = "Documents", security(("bearer_auth" = [])), responses((status = 200, description = "Upload size limits", content_type = "application/json"))))]
pub async fn upload_limits(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "max_upload_size_mb": state.config.server.max_upload_size_mb,
    }))
}

#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/documents", tag = "Documents", security(("bearer_auth" = [])), request_body(content_type = "multipart/form-data", description = "File upload"), responses((status = 200, body = DocumentResponse), (status = 413, description = "File too large"))))]
pub async fn upload(
    State(state): State<AppState>,
    claims: Claims,
    mut multipart: Multipart,
) -> Result<Json<DocumentResponse>, AppError> {
    require_maintainer(&claims)?;

    if !state.config.features.document_upload_enabled {
        return Err(AppError::FeatureDisabled("Document upload".to_string()));
    }

    // Require an embedding API key before accepting the upload
    let embedding_provider = state.config.llm.default_provider.clone();
    let api_key = state
        .settings_repo
        .get_api_key(&claims.sub, &embedding_provider)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    if api_key.is_empty() {
        return Err(AppError::Validation(format!(
            "No API key configured for embedding provider '{}'. Add one in Settings before uploading.",
            embedding_provider
        )));
    }

    let max_file_size = state.config.server.max_upload_size_mb * 1024 * 1024;

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("Invalid multipart data: {e}")))?
        .ok_or_else(|| AppError::Validation("No file provided".to_string()))?;

    let original_filename = field
        .file_name()
        .unwrap_or("unnamed.txt")
        .to_string();

    let content_type = field
        .content_type()
        .unwrap_or("application/octet-stream")
        .to_string();

    if !crate::services::text_extract::is_supported(&content_type, &original_filename) {
        return Err(AppError::Validation(format!(
            "Unsupported file type. Supported: PDF, DOCX, XLSX, XML, CSV, TXT, MD"
        )));
    }

    let data = field
        .bytes()
        .await
        .map_err(|e| AppError::Validation(format!("Failed to read file: {e}")))?;

    if data.len() > max_file_size {
        return Err(AppError::PayloadTooLarge(
            state.config.server.max_upload_size_mb,
        ));
    }

    let size_bytes = data.len() as i64;

    // Create document record first
    let doc = state
        .document_repo
        .create(
            &claims.sub,
            &original_filename,
            "", // placeholder, will update after upload
            &content_type,
            size_bytes,
        )
        .await?;

    let minio_key = StorageService::generate_key(&claims.sub, &doc.id, &original_filename);

    // Upload to MinIO
    let storage = state.storage.clone();
    let upload_data = data.to_vec();
    let ct = content_type.clone();
    let key = minio_key.clone();

    storage
        .upload(&key, upload_data, &ct)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Update status to processing
    state
        .document_repo
        .update_status(&doc.id, &DocumentStatus::Processing, None)
        .await?;

    // Spawn background processing task
    let doc_id = doc.id.clone();
    let doc_repo = state.document_repo.clone();
    let storage_clone = state.storage.clone();
    let vector_service = state.vector_service.clone();
    let chunk_repo = state.chunk_repo.clone();
    let embedding_model = state.config.llm.default_embedding_model.clone();
    let file_content_type = content_type.clone();
    let file_name = original_filename.clone();

    tokio::spawn(async move {
        match process_document(
            &storage_clone,
            &key,
            &doc_id,
            &file_content_type,
            &file_name,
            &vector_service,
            &chunk_repo,
            &embedding_provider,
            &embedding_model,
            &api_key,
        )
        .await
        {
            Ok(()) => {
                let _ = doc_repo
                    .update_status(&doc_id, &DocumentStatus::Ready, None)
                    .await;
                tracing::info!("Document {doc_id} processed successfully");
            }
            Err(e) => {
                let msg = format!("{e:#}");
                let _ = doc_repo
                    .update_status(&doc_id, &DocumentStatus::Failed, Some(&msg))
                    .await;
                tracing::error!("Document {doc_id} processing failed: {msg}");
            }
        }
    });

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "document.upload",
        Some("document"),
        Some(&doc.id),
        &format!("Uploaded document '{}'", original_filename),
        None,
        None,
    );

    // Return with processing status
    let updated_doc = state
        .document_repo
        .find_by_id(&doc.id)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Document disappeared")))?;

    Ok(Json(updated_doc.into()))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/documents", tag = "Documents", security(("bearer_auth" = [])), responses((status = 200, body = Vec<DocumentResponse>))))]
pub async fn list(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<DocumentResponse>>, AppError> {
    require_maintainer(&claims)?;
    let docs = state.document_repo.find_by_user(&claims.sub).await?;
    Ok(Json(docs.into_iter().map(|d| d.into()).collect()))
}

#[cfg_attr(feature = "openapi", utoipa::path(get, path = "/api/documents/{id}", tag = "Documents", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Document ID")), responses((status = 200, body = DocumentResponse))))]
pub async fn get_document(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<DocumentResponse>, AppError> {
    require_maintainer(&claims)?;
    let doc = state
        .document_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    if doc.user_id != claims.sub && claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    Ok(Json(doc.into()))
}

#[cfg_attr(feature = "openapi", utoipa::path(delete, path = "/api/documents/{id}", tag = "Documents", security(("bearer_auth" = [])), params(("id" = String, Path, description = "Document ID")), responses((status = 200))))]
pub async fn delete_document(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<(), AppError> {
    require_maintainer(&claims)?;
    let doc = state
        .document_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    if doc.user_id != claims.sub && claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    // Delete vectors from Qdrant and chunk records
    let point_ids = state.chunk_repo.delete_by_source("document", &id).await?;
    if !point_ids.is_empty() {
        if let Err(e) = state.vector_service.delete_points(point_ids).await {
            tracing::error!("Failed to delete vectors for document {id}: {e}");
        }
    }

    // Delete from MinIO
    state
        .storage
        .delete(&doc.minio_key)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Delete record
    state.document_repo.delete(&id).await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "document.delete",
        Some("document"),
        Some(&id),
        "Deleted document",
        None,
        None,
    );

    Ok(())
}

/// Rescan all documents: re-extract, re-chunk, and re-embed into the vector database.
#[cfg_attr(feature = "openapi", utoipa::path(post, path = "/api/documents/rescan", tag = "Documents", security(("bearer_auth" = [])), responses((status = 200, description = "Rescan started"))))]
pub async fn rescan(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&claims)?;

    // Require an embedding API key before rescanning
    let embedding_provider = state.config.llm.default_provider.clone();
    let api_key = state
        .settings_repo
        .get_api_key(&claims.sub, &embedding_provider)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    if api_key.is_empty() {
        return Err(AppError::Validation(format!(
            "No API key configured for embedding provider '{}'. Add one in Settings before rescanning.",
            embedding_provider
        )));
    }

    let docs = state.document_repo.find_all_ready().await?;
    let total = docs.len();

    let vector_service = state.vector_service.clone();
    let chunk_repo = state.chunk_repo.clone();
    let storage = state.storage.clone();
    let embedding_model = state.config.llm.default_embedding_model.clone();

    tokio::spawn(async move {
        tracing::info!("Starting rescan of {total} documents");

        for doc in docs {
            // Delete existing chunks for this document
            let old_point_ids = chunk_repo.delete_by_source("document", &doc.id).await.unwrap_or_default();
            if !old_point_ids.is_empty() {
                let _ = vector_service.delete_points(old_point_ids).await;
            }

            // Re-process
            if let Err(e) = process_document(
                &storage,
                &doc.minio_key,
                &doc.id,
                &doc.content_type,
                &doc.original_filename,
                &vector_service,
                &chunk_repo,
                &embedding_provider,
                &embedding_model,
                &api_key,
            )
            .await
            {
                tracing::error!("Rescan failed for document {}: {e:#}", doc.id);
            }
        }

        tracing::info!("Rescan completed for {total} documents");
    });

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "document.rescan",
        None,
        None,
        &format!("Started rescan of {total} documents"),
        None,
        None,
    );

    Ok(Json(serde_json::json!({
        "message": format!("Rescan started for {total} documents"),
        "total": total,
    })))
}

async fn process_document(
    storage: &StorageService,
    minio_key: &str,
    doc_id: &str,
    content_type: &str,
    filename: &str,
    vector_service: &Arc<VectorService>,
    chunk_repo: &DocumentChunkRepository,
    embedding_provider: &str,
    embedding_model: &str,
    api_key: &str,
) -> anyhow::Result<()> {
    // Download from MinIO
    let file_bytes = storage.download(minio_key).await?;

    // Extract text based on file type
    let text = crate::services::text_extract::extract_text(&file_bytes, content_type, filename)?;

    // Chunk text for embedding
    let chunks = crate::services::text_extract::chunk_text(&text, 200, 30);

    if chunks.is_empty() {
        tracing::info!("Document {doc_id}: no text chunks to embed");
        return Ok(());
    }

    // Generate embeddings
    if api_key.is_empty() {
        anyhow::bail!("No API key configured for embedding provider '{embedding_provider}'");
    }

    let embeddings_client =
        crate::services::llm_provider::create_embeddings_client(embedding_provider, api_key)?;

    let model = rig::client::embeddings::EmbeddingsClientDyn::embedding_model(
        embeddings_client.as_ref(),
        embedding_model,
    );

    let texts: Vec<String> = chunks.clone();
    let embeddings = model
        .embed_texts(texts)
        .await
        .map_err(|e| anyhow::anyhow!("Embedding error: {e}"))?;

    // Prepare data for Qdrant and database
    let mut qdrant_data = Vec::with_capacity(chunks.len());
    let mut db_data = Vec::with_capacity(chunks.len());

    for (i, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        let point_id = uuid::Uuid::new_v4().to_string();

        qdrant_data.push((point_id.clone(), embedding.vec.clone(), chunk.clone()));
        db_data.push((
            "document".to_string(),
            doc_id.to_string(),
            i as i32,
            chunk.clone(),
            point_id,
        ));
    }

    // Upsert to Qdrant
    vector_service.upsert_chunks(qdrant_data).await?;

    // Save chunk metadata to database
    chunk_repo.create_batch(&db_data).await?;

    tracing::info!(
        "Document {doc_id}: embedded {} chunks into Qdrant",
        chunks.len()
    );

    Ok(())
}
