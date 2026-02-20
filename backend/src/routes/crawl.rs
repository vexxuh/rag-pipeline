use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use std::sync::Arc;

use crate::db::models::crawl_job::CrawlJob;
use crate::db::models::document_chunk::DocumentChunkRepository;
use crate::errors::AppError;
use crate::middleware::auth::{require_maintainer, Claims};
use crate::services::audit;
use crate::services::vector::VectorService;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct StartCrawlRequest {
    pub url: String,
    pub crawl_type: String, // "sitemap" or "full"
}

pub async fn start_crawl(
    State(state): State<AppState>,
    claims: Claims,
    Json(payload): Json<StartCrawlRequest>,
) -> Result<Json<CrawlJob>, AppError> {
    require_maintainer(&claims)?;

    if !state.config.features.web_crawl_enabled {
        return Err(AppError::FeatureDisabled("Web crawling".to_string()));
    }

    if payload.url.trim().is_empty() {
        return Err(AppError::Validation("URL is required".to_string()));
    }

    if payload.crawl_type != "sitemap" && payload.crawl_type != "full" {
        return Err(AppError::Validation(
            "crawl_type must be 'sitemap' or 'full'".to_string(),
        ));
    }

    // Validate URL
    url::Url::parse(&payload.url)
        .map_err(|_| AppError::Validation("Invalid URL".to_string()))?;

    let job = state
        .crawl_repo
        .create(&claims.sub, &payload.url, &payload.crawl_type)
        .await?;

    audit::log(
        &state.audit_log_repo,
        Some(&claims.sub),
        "crawl.start",
        Some("crawl_job"),
        Some(&job.id),
        &format!("Started {} crawl of {}", payload.crawl_type, payload.url),
        None,
        None,
    );

    // Spawn background crawl task
    let job_id = job.id.clone();
    let url = payload.url.clone();
    let crawl_type = payload.crawl_type.clone();
    let crawler = state.crawler.clone();
    let crawl_repo = state.crawl_repo.clone();
    let vector_service = state.vector_service.clone();
    let chunk_repo = state.chunk_repo.clone();
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
        // Update to running
        let _ = crawl_repo
            .update_status(&job_id, "running", None, None, None)
            .await;

        let result = match crawl_type.as_str() {
            "sitemap" => {
                run_crawl(
                    &crawler,
                    &crawl_repo,
                    &job_id,
                    &url,
                    true,
                    &vector_service,
                    &chunk_repo,
                    &embedding_provider,
                    &embedding_model,
                    &api_key,
                )
                .await
            }
            "full" => {
                run_crawl(
                    &crawler,
                    &crawl_repo,
                    &job_id,
                    &url,
                    false,
                    &vector_service,
                    &chunk_repo,
                    &embedding_provider,
                    &embedding_model,
                    &api_key,
                )
                .await
            }
            _ => Err(anyhow::anyhow!("Invalid crawl type")),
        };

        match result {
            Ok(()) => {
                tracing::info!("Crawl job {job_id} completed");
            }
            Err(e) => {
                let msg = format!("{e:#}");
                let _ = crawl_repo
                    .update_status(&job_id, "failed", None, None, Some(&msg))
                    .await;
                tracing::error!("Crawl job {job_id} failed: {msg}");
            }
        }
    });

    Ok(Json(job))
}

pub async fn get_crawl_job(
    State(state): State<AppState>,
    claims: Claims,
    Path(id): Path<String>,
) -> Result<Json<CrawlJob>, AppError> {
    require_maintainer(&claims)?;
    let job = state
        .crawl_repo
        .find_by_id(&id)
        .await?
        .ok_or_else(|| AppError::NotFound("Crawl job not found".to_string()))?;

    if job.user_id != claims.sub && claims.role != "admin" {
        return Err(AppError::Forbidden);
    }

    Ok(Json(job))
}

pub async fn list_crawl_jobs(
    State(state): State<AppState>,
    claims: Claims,
) -> Result<Json<Vec<CrawlJob>>, AppError> {
    require_maintainer(&claims)?;
    let jobs = state.crawl_repo.find_by_user(&claims.sub).await?;
    Ok(Json(jobs))
}

async fn run_crawl(
    crawler: &crate::services::crawler::CrawlerService,
    crawl_repo: &crate::db::models::crawl_job::CrawlJobRepository,
    job_id: &str,
    url: &str,
    is_sitemap: bool,
    vector_service: &Arc<VectorService>,
    chunk_repo: &DocumentChunkRepository,
    embedding_provider: &str,
    embedding_model: &str,
    api_key: &str,
) -> anyhow::Result<()> {
    let urls = if is_sitemap {
        crawler.crawl_sitemap(url).await?
    } else {
        crawler.crawl_full_site(url).await?
    };

    let count = urls.len() as i64;
    crawl_repo
        .update_status(job_id, "running", Some(count), Some(0), None)
        .await?;

    let pages = crawler.fetch_pages(urls).await;
    let successful_pages: Vec<_> = pages.into_iter().filter_map(|r| r.ok()).collect();
    let processed = successful_pages.len() as i64;

    // Embed page content if we have an API key
    if !api_key.is_empty() && !successful_pages.is_empty() {
        if let Err(e) = embed_crawled_pages(
            &successful_pages,
            job_id,
            vector_service,
            chunk_repo,
            embedding_provider,
            embedding_model,
            api_key,
        )
        .await
        {
            tracing::error!("Failed to embed crawled pages for job {job_id}: {e:#}");
        }
    } else if api_key.is_empty() {
        tracing::warn!(
            "Crawl job {job_id}: no API key for embedding provider '{embedding_provider}', skipping embedding"
        );
    }

    crawl_repo
        .update_status(job_id, "completed", None, Some(processed), None)
        .await?;
    Ok(())
}

async fn embed_crawled_pages(
    pages: &[crate::services::crawler::CrawledPage],
    job_id: &str,
    vector_service: &Arc<VectorService>,
    chunk_repo: &DocumentChunkRepository,
    embedding_provider: &str,
    embedding_model_name: &str,
    api_key: &str,
) -> anyhow::Result<()> {
    let embeddings_client =
        crate::services::llm_provider::create_embeddings_client(embedding_provider, api_key)?;

    let model = rig::client::embeddings::EmbeddingsClientDyn::embedding_model(
        embeddings_client.as_ref(),
        embedding_model_name,
    );

    let mut all_chunks: Vec<String> = Vec::new();
    let mut chunk_metadata: Vec<(usize, i32)> = Vec::new(); // (page_index, chunk_index)

    for (page_idx, page) in pages.iter().enumerate() {
        let chunks = crate::services::pdf::chunk_text(&page.content, 200, 30);
        for (chunk_idx, chunk) in chunks.into_iter().enumerate() {
            all_chunks.push(chunk);
            chunk_metadata.push((page_idx, chunk_idx as i32));
        }
    }

    if all_chunks.is_empty() {
        return Ok(());
    }

    // Embed in batches to avoid API limits
    let batch_size = 100;
    let mut qdrant_data = Vec::new();
    let mut db_data = Vec::new();

    for batch_start in (0..all_chunks.len()).step_by(batch_size) {
        let batch_end = (batch_start + batch_size).min(all_chunks.len());
        let batch: Vec<String> = all_chunks[batch_start..batch_end].to_vec();

        let embeddings = model
            .embed_texts(batch)
            .await
            .map_err(|e| anyhow::anyhow!("Embedding error: {e}"))?;

        for (i, embedding) in embeddings.iter().enumerate() {
            let global_idx = batch_start + i;
            let point_id = uuid::Uuid::new_v4().to_string();

            qdrant_data.push((
                point_id.clone(),
                embedding.vec.clone(),
                all_chunks[global_idx].clone(),
            ));
            db_data.push((
                "crawl_page".to_string(),
                job_id.to_string(),
                chunk_metadata[global_idx].1,
                all_chunks[global_idx].clone(),
                point_id,
            ));
        }
    }

    // Upsert to Qdrant
    vector_service.upsert_chunks(qdrant_data).await?;

    // Save to database
    chunk_repo.create_batch(&db_data).await?;

    tracing::info!(
        "Crawl job {job_id}: embedded {} chunks from {} pages into Qdrant",
        all_chunks.len(),
        pages.len()
    );

    Ok(())
}
