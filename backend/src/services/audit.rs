use crate::db::models::audit_log::AuditLogRepository;

pub fn log(
    repo: &AuditLogRepository,
    user_id: Option<&str>,
    event_type: &str,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    description: &str,
    ip_address: Option<&str>,
    metadata: Option<serde_json::Value>,
) {
    let repo = repo.clone();
    let user_id = user_id.map(|s| s.to_string());
    let event_type = event_type.to_string();
    let resource_type = resource_type.map(|s| s.to_string());
    let resource_id = resource_id.map(|s| s.to_string());
    let description = description.to_string();
    let ip_address = ip_address.map(|s| s.to_string());

    tokio::spawn(async move {
        if let Err(e) = repo
            .create(
                user_id.as_deref(),
                &event_type,
                resource_type.as_deref(),
                resource_id.as_deref(),
                &description,
                ip_address.as_deref(),
                metadata,
            )
            .await
        {
            tracing::error!("Failed to write audit log: {e}");
        }
    });
}
