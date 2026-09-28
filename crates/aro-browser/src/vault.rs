//! Credential vault: OS keyring storage + blind form fill.
//!
//! Invariants (audited in Phase 3):
//! - secrets are kept in [`SecretString`] (redacted `Debug`, zeroized on drop);
//! - `fill_login_form` types values via CDP and returns only `()` — the
//!   values never flow into logs, snapshots, or model context.

use keyring::Entry;
use zeroize::Zeroize;

const KEYRING_SERVICE: &str = "aro-browser-vault";

/// A secret that never prints and wipes itself on drop.
#[derive(Clone)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretString([redacted])")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl serde::Serialize for SecretString {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str("[redacted]")
    }
}

impl<'de> serde::Deserialize<'de> for SecretString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Inbound only: values arrive from trusted IPC (user vault unlock),
        // never from model output. The model-facing schemas must not include
        // secret fields at all.
        let value = String::deserialize(deserializer)?;
        Ok(Self(value))
    }
}

fn entry_for(account: &str) -> Result<Entry, crate::act::BrowserError> {
    Entry::new(KEYRING_SERVICE, account)
        .map_err(|err| crate::act::BrowserError::Vault(err.to_string()))
}

/// Persist one credential in the OS keyring (Credential Manager / Keychain /
/// Secret Service). `account` is typically the login domain.
pub fn save_credential(account: &str, secret: &SecretString) -> Result<(), crate::act::BrowserError> {
    let entry = entry_for(account)?;
    entry
        .set_password(secret.as_str())
        .map_err(|err| crate::act::BrowserError::Vault(err.to_string()))
}

/// Load one credential from the OS keyring.
pub fn load_credential(account: &str) -> Result<SecretString, crate::act::BrowserError> {
    let entry = entry_for(account)?;
    entry
        .get_password()
        .map(SecretString::new)
        .map_err(|err| crate::act::BrowserError::Vault(err.to_string()))
}

/// Delete one credential from the OS keyring.
pub fn delete_credential(account: &str) -> Result<(), crate::act::BrowserError> {
    let entry = entry_for(account)?;
    entry
        .delete_credential()
        .map_err(|err| crate::act::BrowserError::Vault(err.to_string()))
}

/// Type username + password into live fields and submit-neutral (no Enter).
/// Values are used once and never returned, logged, or snapshotted.
pub async fn fill_login_form(
    page: &chromiumoxide::Page,
    username_ref: &str,
    password_ref: &str,
    username: &str,
    password: &str,
) -> Result<(), crate::act::BrowserError> {
    use crate::observe::find_by_ref;
    if username.is_empty() || password.is_empty() {
        return Err(crate::act::BrowserError::InvalidInput(
            "empty credential material".to_string(),
        ));
    }
    let user_el = find_by_ref(page, username_ref)
        .await
        .map_err(|_| crate::act::BrowserError::NotObserved(format!("no element {username_ref}")))?;
    user_el.click().await?;
    user_el.type_str(username).await?;
    let pass_el = find_by_ref(page, password_ref)
        .await
        .map_err(|_| crate::act::BrowserError::NotObserved(format!("no element {password_ref}")))?;
    pass_el.click().await?;
    pass_el.type_str(password).await?;
    Ok(())
}
