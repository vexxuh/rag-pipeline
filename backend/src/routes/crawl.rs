use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::db::models::crawl_job::CrawlJob;
use crate::errors::AppError;
use crate::middleware::auth::Claims;
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
        .create(&claims.sub, &payload.url, &payload.crawl_type)?;

    // Spawn background crawl task
    let job_id = job.id.clone();
    let url = payload.url.clone();
    let crawl_type = payload.crawl_type.clone();
    let crawler = state.crawler.clone();
    let crawl_repo = state.crawl_repo.clone();

    tokio::spawn(async move {
        // Update to running
        let _ = crawl_repo.update_status(&job_id, "running", None, None, None);

        let result = match crawl_type.as_str() {
            "sitemap" => run_sitemap_crawl(&crawler, &crawl_repo, &job_id, &url).await,
            "full" => run_full_crawl(&crawler, &crawl_repo, &job_id, &url).await,
            _ => Err(anyhow::anyhow!("Invalid crawl type")),
        };

        match result {
            Ok(()) => {
                tracing::info!("Crawl job {job_id} completed");
            }
            Err(e) => {
                let msg = format!("{e:#}");
                let _ = crawl_repo.update_status(&job_id, "failed", None, None, Some(&msg));
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
    let job = state
        .crawl_repo
        .find_by_id(&id)?
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
    let jobs = state.crawl_repo.find_by_user(&claims.sub)?;
    Ok(Json(jobs))
}

async fn run_sitemap_crawl(
    crawler: &crate::services::crawler::CrawlerService,
    crawl_repo: &crate::db::models::crawl_job::CrawlJobRepository,
    job_id: &str,
    url: &str,
) -> anyhow::Result<()> {
    let urls = crawler.crawl_sitemap(url).await?;
    let count = urls.len() as i64;
    crawl_repo.update_status(job_id, "running", Some(count), Some(0), None)?;

    let pages = crawler.fetch_pages(urls).await;
    let processed = pages.iter().filter(|r| r.is_ok()).count() as i64;

    // TODO: Phase 4 integration - chunk and embed page content

    crawl_repo.update_status(job_id, "completed", None, Some(processed), None)?;
    Ok(())
}

async fn run_full_crawl(
    crawler: &crate::services::crawler::CrawlerService,
    crawl_repo: &crate::db::models::crawl_job::CrawlJobRepository,
    job_id: &str,
    url: &str,
) -> anyhow::Result<()> {
    let urls = crawler.crawl_full_site(url).await?;
    let count = urls.len() as i64;
    crawl_repo.update_status(job_id, "running", Some(count), Some(0), None)?;

    let pages = crawler.fetch_pages(urls).await;
    let processed = pages.iter().filter(|r| r.is_ok()).count() as i64;

    // TODO: Phase 4 integration - chunk and embed page content

    crawl_repo.update_status(job_id, "completed", None, Some(processed), None)?;
    Ok(())
}
