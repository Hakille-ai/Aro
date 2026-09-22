use crate::state::AppState;
use serde_json::Value;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub async fn billing_open_payment(app: tauri::AppHandle, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|_| "invalid payment URL".to_string())?;
    if parsed.scheme() != "https"
        || parsed.port().is_some()
        || !matches!(
            parsed.host_str(),
            Some("checkout.stripe.com" | "billing.stripe.com")
        )
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("untrusted payment URL".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn billing_request(
    state: State<'_, AppState>,
    method: String,
    path: String,
    body: Option<Value>,
) -> Result<Value, String> {
    if method == "GET" && path == "/billing/catalog" {
        return state
            .cloud
            .billing_request("", &method, &path, None)
            .await
            .map_err(|e| e.to_string());
    }
    let session = state
        .refresh_cloud_session_from_keyring()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Connectez-vous pour gérer votre abonnement.".to_string())?;
    state
        .cloud
        .billing_request(&session.access_token, &method, &path, body)
        .await
        .map_err(|e| e.to_string())
}
