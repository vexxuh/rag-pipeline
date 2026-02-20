use axum::{
    extract::{Multipart, Path, State},
    Json,
};

use crate::db::models::document::DocumentStatus;
use crate::dto::document::DocumentResponse;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
use crate::services::storage::StorageService;
use crate::state::AppState;

const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50 MB

pub async fn upload(
    State(state): State<AppState>,
    claims: Claims,
    mut multipart: Multipart,
) -> Result<Json<DocumentResponse>, AppError> {
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
    let doc = state.document_repo.create(
        &claims.sub,
        &original_filename,
        "", // placeholder, will update after upload
        &content_type,
        size_bytes,
    )?;

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
        .update_status(&doc.id, &DocumentStatus::Processing, None)?;

    // Spawn background processing task
    let doc_id = doc.id.clone();
    let doc_repo = state.document_repo.clone();
    let storage_clone = state.storage.clone();

    tokio::spawn(async move {
        match process_document(&storage_clone, &key, &doc_id).await {
            Ok(()) => {
                let _ = doc_repo.update_status(&doc_id, &DocumentStatus::Ready, None);
                tracing::info!("Document {doc_id} processed successfully");
            }
            Err(e) => {
                let msg = format!("{e:#}");
                let _ = doc_repo.update_status(&doc_id, &DocumentStatus::Failed, Some(&msg));
                tracing::error!("Document {doc_id} processing failed: {msg}");
            }
        }
    });

    // Return with processing status
    let updated_doc = state
        .document_repo
        .find_by_id(&doc.id)?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("Document disappeared")))?;

    Ok(Json(updated_doc.into()))
}

pub async fn list(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<DocumentResponse>>, AppError> {
    let docs = state.document_repo.find_by_user(&claims.sub)?;
    Ok(Json(docs.into_iter().map(|d| d.into()).collect()))
}

pub async fn get_document(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<DocumentResponse>, AppError> {
    let doc = state
        .document_repo
        .find_by_id(&id)?
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
    let doc = state
        .document_repo
        .find_by_id(&id)?
        .ok_or_else(|| AppError::NotFound("Document not found".to_string()))?;

    if doc.user_id != claims.sub && claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    // Delete from MinIO
    state
        .storage
        .delete(&doc.minio_key)
        .await
        .map_err(|e| AppError::Internal(e))?;

    // Delete record
    state.document_repo.delete(&id)?;

    Ok(())
}

async fn process_document(
    storage: &StorageService,
    minio_key: &str,
    _doc_id: &str,
) -> anyhow::Result<()> {
    // Download from MinIO
    let pdf_bytes = storage.download(minio_key).await?;

    // Extract text
    let text = crate::services::pdf::extract_text(&pdf_bytes)?;

    // Chunk text for embedding (will be used in Phase 4)
    let _chunks = crate::services::pdf::chunk_text(&text, 200, 30);

    // TODO: Phase 4 - Generate embeddings and store in Qdrant

    Ok(())
}
