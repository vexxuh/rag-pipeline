use utoipa::openapi::security::{Http, HttpAuthScheme, ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::db::models::admin_config::{AddModelRequest, AdminModel, AdminProvider};
use crate::db::models::audit_log::AuditLog;
use crate::db::models::conversation::{Conversation, ConversationWithUser, Message};
use crate::db::models::crawl_job::CrawlJob;
use crate::db::models::document::DocumentStatus;
use crate::db::models::embed_key::{EmbedKey, UpdateEmbedKeyRequest};
use crate::db::models::settings::{ApiKeyEntry, LlmPreferences};
use crate::db::models::user::UserRole;
use crate::dto::auth::{
    AuthResponse, InviteRequest, InviteResponse, LoginRequest, SetupRequest, UpdateRoleRequest,
    UserResponse,
};
use crate::dto::document::DocumentResponse;
use crate::errors::ErrorResponse;
use crate::routes::admin_audit::AuditLogsResponse;
use crate::routes::admin_config::ToggleRequest;
use crate::routes::admin_embed::{CreateEmbedKeyRequest, CreateEmbedKeyResponse};
use crate::routes::admin_logs::{LogDetailResponse, LogsResponse};
use crate::routes::chat::{ConversationWithMessages, CreateConversationRequest, SendMessageRequest};
use crate::routes::crawl::StartCrawlRequest;
use crate::routes::settings::SetApiKeyRequest;
use crate::routes::widget::{
    CreateWidgetConversationRequest, WidgetConfigResponse, WidgetSendMessageRequest,
};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "bearer_auth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Bearer)),
        );
        components.add_security_scheme(
            "embed_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("x-embed-key"))),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "RAG Pipeline API",
        version = "0.1.0",
        description = "RAG Pipeline backend — document upload, chat, admin, and embeddable widget APIs."
    ),
    modifiers(&SecurityAddon),
    paths(
        // Public
        crate::routes::health::health_check,
        crate::routes::auth::login,
        crate::routes::auth::setup,
        // Auth (protected)
        crate::routes::auth::me,
        // Conversations
        crate::routes::chat::create_conversation,
        crate::routes::chat::list_conversations,
        crate::routes::chat::get_conversation,
        crate::routes::chat::delete_conversation,
        crate::routes::chat::send_message,
        // Documents
        crate::routes::documents::upload_limits,
        crate::routes::documents::upload,
        crate::routes::documents::list,
        crate::routes::documents::get_document,
        crate::routes::documents::delete_document,
        crate::routes::documents::rescan,
        // Crawl
        crate::routes::crawl::start_crawl,
        crate::routes::crawl::list_crawl_jobs,
        crate::routes::crawl::get_crawl_job,
        // Settings
        crate::routes::settings::list_providers,
        crate::routes::settings::list_models_for_provider,
        crate::routes::settings::list_api_keys,
        crate::routes::settings::set_api_key,
        crate::routes::settings::delete_api_key,
        crate::routes::settings::get_preferences,
        crate::routes::settings::update_preferences,
        // Admin — Users
        crate::routes::admin::list_users,
        crate::routes::admin::update_user_role,
        crate::routes::admin::delete_user,
        crate::routes::admin::invite_user,
        crate::routes::admin::list_invites,
        // Admin — Logs
        crate::routes::admin_logs::list_conversation_logs,
        crate::routes::admin_logs::get_conversation_log,
        // Admin — Config
        crate::routes::admin_config::list_providers,
        crate::routes::admin_config::toggle_provider,
        crate::routes::admin_config::list_models,
        crate::routes::admin_config::add_model,
        crate::routes::admin_config::remove_model,
        crate::routes::admin_config::set_default_model,
        // Admin — Audit
        crate::routes::admin_audit::list_audit_logs,
        // Admin — Embed keys
        crate::routes::admin_embed::create_key,
        crate::routes::admin_embed::list_keys,
        crate::routes::admin_embed::get_key,
        crate::routes::admin_embed::update_key,
        crate::routes::admin_embed::delete_key,
        crate::routes::admin_embed::toggle_key,
        // Widget
        crate::routes::widget::get_config,
        crate::routes::widget::create_conversation,
        crate::routes::widget::list_conversations,
        crate::routes::widget::get_messages,
        crate::routes::widget::send_message,
    ),
    components(
        schemas(
            // Auth
            LoginRequest, SetupRequest, AuthResponse, UserResponse, UserRole,
            InviteRequest, InviteResponse, UpdateRoleRequest,
            // Conversations
            Conversation, Message, ConversationWithMessages, ConversationWithUser,
            CreateConversationRequest, SendMessageRequest,
            // Documents
            DocumentResponse, DocumentStatus,
            // Crawl
            CrawlJob, StartCrawlRequest,
            // Settings
            AdminProvider, AdminModel, AddModelRequest, ToggleRequest,
            ApiKeyEntry, LlmPreferences, SetApiKeyRequest,
            // Admin logs
            LogsResponse, LogDetailResponse, AuditLogsResponse, AuditLog,
            // Embed keys
            EmbedKey, UpdateEmbedKeyRequest, CreateEmbedKeyRequest, CreateEmbedKeyResponse,
            // Widget
            WidgetConfigResponse, CreateWidgetConversationRequest, WidgetSendMessageRequest,
            // Errors
            ErrorResponse,
        )
    ),
    tags(
        (name = "Health", description = "Health check"),
        (name = "Auth", description = "Authentication and account setup"),
        (name = "Chat", description = "Conversations and messages"),
        (name = "Documents", description = "Document upload and management"),
        (name = "Crawl", description = "Web crawling"),
        (name = "Settings", description = "User settings, API keys, and LLM preferences"),
        (name = "Admin - Users", description = "User and invite management (admin only)"),
        (name = "Admin - Logs", description = "Conversation and audit log viewing (admin only)"),
        (name = "Admin - Config", description = "Provider and model configuration (admin only)"),
        (name = "Admin - Embed", description = "Embed key management (admin only)"),
        (name = "Widget", description = "Embeddable chat widget API"),
    )
)]
pub struct ApiDoc;
