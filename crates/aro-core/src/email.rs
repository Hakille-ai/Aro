use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<String>,
    pub from_email: String,
    pub from_name: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            smtp_host: None,
            smtp_port: Some(587),
            smtp_user: None,
            smtp_password: None,
            from_email: "noreply@aro-ai.com".into(),
            from_name: "ARO Intelligence".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordResetToken {
    pub token: String,
    pub email: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub used: bool,
}

/// Generates a Google/Apple level responsive HTML Password Reset Email template
pub fn render_password_reset_email(reset_url: &str, user_email: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Réinitialisation de votre mot de passe ARO</title>
  <style>
    body {{
      margin: 0;
      padding: 0;
      background-color: #0b0f19;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif;
      color: #f8fafc;
      -webkit-font-smoothing: antialiased;
    }}
    .email-container {{
      max-width: 560px;
      margin: 40px auto;
      background: rgba(15, 23, 42, 0.8);
      border: 1px solid rgba(255, 255, 255, 0.1);
      border-radius: 20px;
      padding: 40px;
      box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
    }}
    .brand-header {{
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 32px;
    }}
    .brand-logo {{
      width: 42px;
      height: 42px;
      border-radius: 12px;
      background: linear-gradient(135deg, #3b82f6 0%, #8b5cf6 100%);
      display: flex;
      align-items: center;
      justify-content: center;
      font-weight: 800;
      font-size: 20px;
      color: #ffffff;
    }}
    .brand-name {{
      font-size: 22px;
      font-weight: 700;
      letter-spacing: -0.5px;
      background: linear-gradient(90deg, #ffffff 0%, #94a3b8 100%);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }}
    h1 {{
      font-size: 24px;
      font-weight: 700;
      margin: 0 0 16px 0;
      color: #ffffff;
    }}
    p {{
      font-size: 15px;
      line-height: 1.6;
      color: #94a3b8;
      margin: 0 0 24px 0;
    }}
    .user-badge {{
      display: inline-block;
      background: rgba(59, 130, 246, 0.1);
      border: 1px solid rgba(59, 130, 246, 0.2);
      color: #60a5fa;
      padding: 4px 12px;
      border-radius: 20px;
      font-size: 13px;
      font-weight: 600;
      margin-bottom: 24px;
    }}
    .btn-container {{
      text-align: center;
      margin: 32px 0;
    }}
    .btn-primary {{
      display: inline-block;
      padding: 14px 32px;
      background: linear-gradient(135deg, #3b82f6 0%, #2563eb 100%);
      color: #ffffff !important;
      text-decoration: none;
      font-weight: 600;
      font-size: 15px;
      border-radius: 12px;
      box-shadow: 0 10px 25px rgba(59, 130, 246, 0.4);
    }}
    .footer-note {{
      font-size: 12px;
      color: #64748b;
      border-top: 1px solid rgba(255, 255, 255, 0.08);
      padding-top: 24px;
      margin-top: 32px;
    }}
    .link-fallback {{
      word-break: break-all;
      color: #3b82f6;
      font-size: 12px;
    }}
  </style>
</head>
<body>
  <div class="email-container">
    <div class="brand-header">
      <div class="brand-logo">A</div>
      <div class="brand-name">ARO Intelligence</div>
    </div>
    
    <h1>Réinitialisation de votre mot de passe</h1>
    
    <div class="user-badge">{user_email}</div>
    
    <p>Nous avons reçu une demande de réinitialisation du mot de passe associé à votre compte ARO. Cliquez sur le bouton ci-dessous pour choisir un nouveau mot de passe sécurisé :</p>
    
    <div class="btn-container">
      <a href="{reset_url}" class="btn-primary" target="_blank">Réinitialiser mon mot de passe</a>
    </div>
    
    <p>Ce lien de sécurité est valide pendant <strong>1 heure</strong>. Si vous n'avez pas demandé cette réinitialisation, vous pouvez ignorer cet e-mail en toute sécurité.</p>
    
    <div class="footer-note">
      <p>Si le bouton ne fonctionne pas, copiez-collez ce lien dans votre navigateur :</p>
      <div class="link-fallback">{reset_url}</div>
      <p style="margin-top: 16px;">© 2026 ARO Intelligence. Tous droits réservés.</p>
    </div>
  </div>
</body>
</html>"#,
        user_email = user_email,
        reset_url = reset_url
    )
}

/// Generates a Google/Apple level responsive HTML Agent Completion Email template
pub fn render_agent_completion_email(
    conversation_title: &str,
    duration_secs: u64,
    summary: &str,
    view_url: &str,
) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Tâche ARO Terminée</title>
  <style>
    body {{
      margin: 0;
      padding: 0;
      background-color: #0b0f19;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      color: #f8fafc;
    }}
    .email-container {{
      max-width: 560px;
      margin: 40px auto;
      background: rgba(15, 23, 42, 0.9);
      border: 1px solid rgba(16, 185, 129, 0.2);
      border-radius: 20px;
      padding: 40px;
    }}
    .header-badge {{
      display: inline-flex;
      align-items: center;
      gap: 6px;
      background: rgba(16, 185, 129, 0.1);
      border: 1px solid rgba(16, 185, 129, 0.3);
      color: #10b981;
      padding: 4px 12px;
      border-radius: 20px;
      font-size: 13px;
      font-weight: 600;
      margin-bottom: 20px;
    }}
    h1 {{
      font-size: 22px;
      font-weight: 700;
      color: #ffffff;
      margin: 0 0 12px 0;
    }}
    .summary-box {{
      background: rgba(0, 0, 0, 0.3);
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 12px;
      padding: 16px;
      margin: 20px 0;
      font-size: 14px;
      color: #cbd5e1;
      line-height: 1.5;
    }}
    .btn-primary {{
      display: inline-block;
      padding: 12px 28px;
      background: linear-gradient(135deg, #10b981 0%, #059669 100%);
      color: #ffffff !important;
      text-decoration: none;
      font-weight: 600;
      border-radius: 10px;
    }}
  </style>
</head>
<body>
  <div class="email-container">
    <div class="header-badge">✓ Tâche Terminée ({duration_secs}s)</div>
    <h1>L'agent ARO a terminé sa tâche</h1>
    <p style="color: #94a3b8;">Conversation : <strong>{conversation_title}</strong></p>
    <div class="summary-box">{summary}</div>
    <div style="text-align: center; margin-top: 28px;">
      <a href="{view_url}" class="btn-primary">Consulter les Résultats</a>
    </div>
  </div>
</body>
</html>"#,
        duration_secs = duration_secs,
        conversation_title = conversation_title,
        summary = summary,
        view_url = view_url
    )
}
