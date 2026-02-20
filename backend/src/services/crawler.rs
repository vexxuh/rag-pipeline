use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Semaphore;
use url::Url;

use crate::config::CrawlerConfig;

pub struct CrawlerService {
    client: reqwest::Client,
    config: CrawlerConfig,
}

pub struct CrawledPage {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
}

impl CrawlerService {
    pub fn new(config: &CrawlerConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.request_timeout_secs))
            .user_agent(&config.user_agent)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            config: config.clone(),
        }
    }

    pub async fn crawl_sitemap(&self, base_url: &str) -> Result<Vec<String>> {
        let sitemap_url = format!("{}/sitemap.xml", base_url.trim_end_matches('/'));

        let body = self
            .client
            .get(&sitemap_url)
            .send()
            .await
            .context("Failed to fetch sitemap")?
            .text()
            .await
            .context("Failed to read sitemap body")?;

        let doc = Html::parse_document(&body);
        let loc_selector = Selector::parse("loc").unwrap();

        let urls: Vec<String> = doc
            .select(&loc_selector)
            .filter_map(|el| el.text().next())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(urls)
    }

    pub async fn crawl_full_site(&self, base_url: &str) -> Result<Vec<String>> {
        let base = Url::parse(base_url).context("Invalid base URL")?;
        let mut visited = HashSet::new();
        let mut to_visit = vec![base_url.to_string()];
        let mut found_urls = Vec::new();

        while let Some(url) = to_visit.pop() {
            if visited.len() >= 200 {
                // Safety limit
                break;
            }
            if visited.contains(&url) {
                continue;
            }
            visited.insert(url.clone());
            found_urls.push(url.clone());

            let Ok(body) = self.fetch_html(&url).await else {
                continue;
            };

            let doc = Html::parse_document(&body);
            let link_selector = Selector::parse("a[href]").unwrap();

            for element in doc.select(&link_selector) {
                if let Some(href) = element.value().attr("href") {
                    if let Ok(resolved) = base.join(href) {
                        let resolved_str = resolved.to_string();
                        if resolved.host_str() == base.host_str()
                            && !visited.contains(&resolved_str)
                            && !resolved_str.contains('#')
                        {
                            to_visit.push(resolved_str);
                        }
                    }
                }
            }
        }

        Ok(found_urls)
    }

    pub async fn fetch_pages(
        &self,
        urls: Vec<String>,
    ) -> Vec<Result<CrawledPage>> {
        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent));
        let mut handles = Vec::new();

        for url in urls {
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let client = self.client.clone();

            let handle = tokio::spawn(async move {
                let result = fetch_and_extract(&client, &url).await;
                drop(permit);
                result
            });

            handles.push(handle);
        }

        let mut results = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(result) => results.push(result),
                Err(e) => results.push(Err(anyhow::anyhow!("Task panicked: {e}"))),
            }
        }

        results
    }

    async fn fetch_html(&self, url: &str) -> Result<String> {
        self.client
            .get(url)
            .send()
            .await
            .context("Failed to fetch page")?
            .text()
            .await
            .context("Failed to read page body")
    }
}

async fn fetch_and_extract(client: &reqwest::Client, url: &str) -> Result<CrawledPage> {
    let body = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch page")?
        .text()
        .await
        .context("Failed to read page body")?;

    let doc = Html::parse_document(&body);

    let title = Selector::parse("title")
        .ok()
        .and_then(|sel| doc.select(&sel).next())
        .and_then(|el| el.text().next())
        .map(|s| s.trim().to_string());

    let content = extract_text_content(&doc);

    Ok(CrawledPage {
        url: url.to_string(),
        title,
        content,
    })
}

fn extract_text_content(doc: &Html) -> String {
    let body_sel = Selector::parse("body").unwrap();
    let skip_sel =
        Selector::parse("script, style, nav, footer, header, aside, iframe, noscript").unwrap();

    let mut text = String::new();

    if let Some(body) = doc.select(&body_sel).next() {
        for node in body.descendants() {
            // Skip unwanted elements
            if let Some(el) = node.value().as_element() {
                if skip_sel.matches(&scraper::ElementRef::wrap(node).unwrap_or_else(|| {
                    // fallback - shouldn't happen for elements
                    doc.select(&body_sel).next().unwrap()
                })) {
                    continue;
                }
                let _ = el;
            }

            if let Some(t) = node.value().as_text() {
                let cleaned = t.trim();
                if !cleaned.is_empty() {
                    text.push_str(cleaned);
                    text.push(' ');
                }
            }
        }
    }

    text.trim().to_string()
}
