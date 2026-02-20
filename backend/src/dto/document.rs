use serde::Serialize;

use crate::db::models::document::{Document, DocumentStatus};

#[derive(Debug, Serialize)]
pub struct DocumentResponse {
    pub id: String,
    pub original_filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub status: DocumentStatus,
    pub error_message: Option<String>,
    pub created_at: String,
    pub processed_at: Option<String>,
}

impl From<Document> for DocumentResponse {
    fn from(doc: Document) -> Self {
        Self {
            id: doc.id,
            original_filename: doc.original_filename,
            content_type: doc.content_type,
            size_bytes: doc.size_bytes,
            status: doc.status,
            error_message: doc.error_message,
            created_at: doc.created_at,
            processed_at: doc.processed_at,
        }
    }
}
