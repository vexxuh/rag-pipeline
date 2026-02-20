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

const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50 MB

pub async fn upload(
    State(state): State<AppState>,
    claims: Claims,
    mut multipart: Multipart,
) -> Result<Json<DocumentResponse>, AppError> {
    require_maintainer(&claims)?;

    if !state.config.features.pdf_upload_enabled {
        return Err(AppError::FeatureDisabled("PDF upload".to_string()));
    }

    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::Validation(format!("Invalid multipart data: {e}")))?
        .ok_or_else(|| AppError::Validation("No file provided".to_string()))?;

    let original_filename = field
        .file_name()
        .unwrap_or("unnamed.pdf")
        .to_string();

    let content_type = field
        .content_type()
        .unwrap_or("application/pdf")
        .to_string();

    if content_type != "application/pdf" {
        return Err(AppError::Validation(
            "Only PDF files are supported".to_string(),
        ));
    }

    let data = field
        .bytes()
        .await
        .map_err(|e| AppError::Validation(format!("Failed to read file: {e}")))?;

    if data.len() > MAX_FILE_SIZE {
        return Err(AppError::Validation(format!(
            "File too large. Maximum size is {} MB",
            MAX_FILE_SIZE / 1024 / 1024
        )));
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
    let embedding_provider = state.config.llm.default_provider.clone();
    let embedding_model = state.config.llm.default_embedding_model.clone();
    // Try to get the user's API key for embeddings
    let api_key = state
        .settings_repo
        .get_api_key(&claims.sub, &embedding_provider)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    tokio::spawn(async move {
        match process_document(
            &storage_clone,
            &key,
            &doc_id,
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

pub async fn list(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<DocumentResponse>>, AppError> {
    require_maintainer(&claims)?;
    let docs = state.document_repo.find_by_user(&claims.sub).await?;
    Ok(Json(docs.into_iter().map(|d| d.into()).collect()))
}

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
pub async fn rescan(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<serde_json::Value>, AppError> {
    require_admin(&claims)?;

    let docs = state.document_repo.find_all_ready().await?;
    let total = docs.len();

    let vector_service = state.vector_service.clone();
    let chunk_repo = state.chunk_repo.clone();
    let storage = state.storage.clone();
    let embedding_provider = state.config.llm.default_provider.clone();
    let embedding_model = state.config.llm.default_embedding_model.clone();
    let api_key = state
        .settings_repo
        .get_api_key(&claims.sub, &embedding_provider)
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

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
    vector_service: &Arc<VectorService>,
    chunk_repo: &DocumentChunkRepository,
    embedding_provider: &str,
    embedding_model: &str,
    api_key: &str,
) -> anyhow::Result<()> {
    // Download from MinIO
    let pdf_bytes = storage.download(minio_key).await?;

    // Extract text
    let text = crate::services::pdf::extract_text(&pdf_bytes)?;

    // Chunk text for embedding
    let chunks = crate::services::pdf::chunk_text(&text, 200, 30);

    if chunks.is_empty() {
        tracing::info!("Document {doc_id}: no text chunks to embed");
        return Ok(());
    }

    // Generate embeddings
    if api_key.is_empty() {
        tracing::warn!("Document {doc_id}: no API key for embedding provider '{embedding_provider}', skipping embedding");
        return Ok(());
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
