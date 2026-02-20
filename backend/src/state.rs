use crate::config::AppConfig;
use crate::db::models::admin_config::AdminConfigRepository;
use crate::db::models::conversation::ConversationRepository;
use crate::db::models::crawl_job::CrawlJobRepository;
use crate::db::models::document::DocumentRepository;
use crate::db::models::invite::InviteRepository;
use crate::db::models::settings::SettingsRepository;
use crate::db::models::user::UserRepository;
use crate::services::crawler::CrawlerService;
use crate::services::email::EmailService;
use crate::services::storage::StorageService;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub db: Pool<SqliteConnectionManager>,
    pub user_repo: UserRepository,
    pub invite_repo: InviteRepository,
    pub document_repo: DocumentRepository,
    pub settings_repo: SettingsRepository,
    pub crawl_repo: CrawlJobRepository,
    pub admin_config_repo: AdminConfigRepository,
    pub conversation_repo: ConversationRepository,
    pub storage: StorageService,
    pub crawler: Arc<CrawlerService>,
    pub email: EmailService,
}

impl AppState {
    pub fn new(
        config: AppConfig,
        db: Pool<SqliteConnectionManager>,
        storage: StorageService,
    ) -> Self {
        let user_repo = UserRepository::new(db.clone());
        let invite_repo = InviteRepository::new(db.clone());
        let document_repo = DocumentRepository::new(db.clone());
        let settings_repo = SettingsRepository::new(db.clone());
        let crawl_repo = CrawlJobRepository::new(db.clone());
        let admin_config_repo = AdminConfigRepository::new(db.clone());
        let conversation_repo = ConversationRepository::new(db.clone());
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
            storage,
            crawler,
            email,
        }
    }
}
