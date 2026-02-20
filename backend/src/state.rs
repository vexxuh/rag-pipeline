use crate::config::AppConfig;
use crate::db::models::admin_config::AdminConfigRepository;
use crate::db::models::audit_log::AuditLogRepository;
use crate::db::models::conversation::ConversationRepository;
use crate::db::models::crawl_job::CrawlJobRepository;
use crate::db::models::document::DocumentRepository;
use crate::db::models::document_chunk::DocumentChunkRepository;
use crate::db::models::embed_key::EmbedKeyRepository;
use crate::db::models::invite::InviteRepository;
use crate::db::models::settings::SettingsRepository;
use crate::db::models::user::UserRepository;
use crate::db::models::widget_session::WidgetSessionRepository;
use crate::services::crawler::CrawlerService;
use crate::services::email::EmailService;
use crate::services::storage::StorageService;
use crate::services::vector::VectorService;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: PgPool,
    pub user_repo: UserRepository,
    pub invite_repo: InviteRepository,
    pub document_repo: DocumentRepository,
    pub settings_repo: SettingsRepository,
    pub crawl_repo: CrawlJobRepository,
    pub admin_config_repo: AdminConfigRepository,
    pub conversation_repo: ConversationRepository,
    pub audit_log_repo: AuditLogRepository,
    pub chunk_repo: DocumentChunkRepository,
    pub embed_key_repo: EmbedKeyRepository,
    pub widget_session_repo: WidgetSessionRepository,
    pub storage: StorageService,
    pub crawler: Arc<CrawlerService>,
    pub vector_service: Arc<VectorService>,
    pub email: EmailService,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: PgPool,
        storage: StorageService,
        vector_service: VectorService,
    ) -> Self {
        let user_repo = UserRepository::new(db.clone());
        let invite_repo = InviteRepository::new(db.clone());
        let document_repo = DocumentRepository::new(db.clone());
        let settings_repo = SettingsRepository::new(db.clone());
        let crawl_repo = CrawlJobRepository::new(db.clone());
        let admin_config_repo = AdminConfigRepository::new(db.clone());
        let conversation_repo = ConversationRepository::new(db.clone());
        let audit_log_repo = AuditLogRepository::new(db.clone());
        let chunk_repo = DocumentChunkRepository::new(db.clone());
        let embed_key_repo = EmbedKeyRepository::new(db.clone());
        let widget_session_repo = WidgetSessionRepository::new(db.clone());
        let crawler = Arc::new(CrawlerService::new(&config.crawler));
        let email = EmailService::new(&config.resend);

        Self {
            config: Arc::new(config),
            db,
            user_repo,
            invite_repo,
            document_repo,
            settings_repo,
            crawl_repo,
            admin_config_repo,
            conversation_repo,
            audit_log_repo,
            chunk_repo,
            embed_key_repo,
            widget_session_repo,
            storage,
            crawler,
            vector_service: Arc::new(vector_service),
            email,
        }
    }
}
