use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    pub database: DatabaseConfig,
    pub minio: MinioConfig,
    pub qdrant: QdrantConfig,
    pub llm: LlmConfig,
    pub features: FeatureFlags,
    pub crawler: CrawlerConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AuthConfig {
    pub enabled: bool,
    pub jwt_secret: String,
    pub jwt_expiry_hours: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseConfig {
    pub path: String,
    pub max_connections: u32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MinioConfig {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub use_ssl: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QdrantConfig {
    pub url: String,
    pub collection_name: String,
    pub vector_size: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct LlmConfig {
    pub default_provider: String,
    pub default_model: String,
    pub default_embedding_model: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct FeatureFlags {
    pub auth_enabled: bool,
    pub pdf_upload_enabled: bool,
    pub web_crawl_enabled: bool,
    pub admin_panel_enabled: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CrawlerConfig {
    pub max_concurrent: usize,
    pub max_depth: usize,
    pub request_timeout_secs: u64,
    pub user_agent: String,
}

impl AppConfig {
    pub fn load() -> Result<Self, ConfigError> {
        let environment = std::env::var("RUN_ENV").unwrap_or_else(|_| "development".into());

        Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(File::with_name(&format!("config/{environment}")).required(false))
            .add_source(Environment::with_prefix("APP").separator("__"))
            .build()?
            .try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_loads() {
        std::env::set_var("RUN_ENV", "development");
        let config = AppConfig::load();
        assert!(config.is_ok(), "Default config should load: {config:?}");

        let config = config.unwrap();
        assert_eq!(config.server.port, 3000);
        assert!(config.features.auth_enabled);
        assert!(config.features.pdf_upload_enabled);
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("APP__SERVER__PORT", "8080");
        std::env::set_var("RUN_ENV", "development");

        let config = AppConfig::load().unwrap();
        assert_eq!(config.server.port, 8080);

        std::env::remove_var("APP__SERVER__PORT");
    }
}
