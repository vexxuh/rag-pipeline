use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;

use crate::config::MinioConfig;

#[derive(Clone)]
pub struct StorageService {
    client: Client,
    bucket: String,
}

impl StorageService {
    pub async fn new(config: &MinioConfig) -> Result<Self> {
        let creds = Credentials::new(
            &config.access_key,
            &config.secret_key,
            None,
            None,
            "minio",
        );

        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()))
            .endpoint_url(&config.endpoint)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(s3_config);

        let service = Self {
            client,
            bucket: config.bucket_name.clone(),
        };

        service.ensure_bucket_exists().await?;

        Ok(service)
    }

    async fn ensure_bucket_exists(&self) -> Result<()> {
        let exists = self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await;

        if exists.is_err() {
            self.client
                .create_bucket()
                .bucket(&self.bucket)
                .send()
                .await
                .context("Failed to create bucket")?;
            tracing::info!("Created bucket: {}", self.bucket);
        }

        Ok(())
    }

    pub async fn upload(
        &self,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<()> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(data))
            .content_type(content_type)
            .send()
            .await
            .context("Failed to upload to MinIO")?;

        Ok(())
    }

    pub async fn download(&self, key: &str) -> Result<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .context("Failed to download from MinIO")?;

        let data = resp
            .body
            .collect()
            .await
            .context("Failed to read response body")?
            .into_bytes()
            .to_vec();

        Ok(data)
    }

    pub async fn delete(&self, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .context("Failed to delete from MinIO")?;

        Ok(())
    }

    pub fn generate_key(user_id: &str, document_id: &str, filename: &str) -> String {
        format!("users/{user_id}/{document_id}/{filename}")
    }
}
