use anyhow::{Context, Result};
use serde::Serialize;

use crate::config::ResendConfig;

#[derive(Clone)]
pub struct EmailService {
    api_key: String,
    from_email: String,
    frontend_url: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct ResendRequest {
    from: String,
    to: Vec<String>,
    subject: String,
    html: String,
}

impl EmailService {
    pub fn new(config: &ResendConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            from_email: config.from_email.clone(),
            frontend_url: config.frontend_url.clone(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn send_invite(&self, email: &str, token: &str) -> Result<()> {
        if self.api_key.is_empty() {
            tracing::warn!("Resend API key not configured, logging invite link instead");
            let link = format!("{}/setup?token={}", self.frontend_url, token);
            tracing::info!("Invite link for {email}: {link}");
            return Ok(());
        }

        let setup_link = format!("{}/setup?token={}", self.frontend_url, token);

        let body = ResendRequest {
            from: self.from_email.clone(),
            to: vec![email.to_string()],
            subject: "You've been invited to RAG Pipeline".to_string(),
            html: format!(
                r#"<!DOCTYPE html>
<html>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2>You've been invited!</h2>
    <p>You've been invited to join the RAG Pipeline application.</p>
    <p>Click the button below to set up your account:</p>
    <a href="{setup_link}"
       style="display: inline-block; padding: 12px 24px; background: #18181b; color: #fafafa;
              text-decoration: none; border-radius: 8px; font-weight: 600;">
        Set Up Account
    </a>
    <p style="margin-top: 20px; color: #71717a; font-size: 14px;">
        Or copy this link: {setup_link}
    </p>
    <p style="color: #71717a; font-size: 12px; margin-top: 40px;">
        This invite expires in 48 hours.
    </p>
</body>
</html>"#
            ),
        };

        let response = self
            .client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .context("Failed to send email via Resend")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Resend API returned {status}: {text}"
            ));
        }

        tracing::info!("Invite email sent to {email}");
        Ok(())
    }
}
