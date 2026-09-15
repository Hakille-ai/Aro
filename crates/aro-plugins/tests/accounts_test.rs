use std::sync::Arc;
use tempfile::tempdir;

use aro_plugins::{
    CreatePluginAccountInput, PluginManager,
};
use aro_skills::SkillRegistry;

#[tokio::test]
async fn test_plugin_manager_multi_account_workflow() {
    let temp_dir = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp_dir.path().to_path_buf(), registry);

    // Initially no accounts
    let accounts = manager.list_accounts(None).unwrap();
    assert!(accounts.is_empty());

    // 1. Add first Google account (Personal) -> should automatically become default
    let google_personal = manager
        .create_account(CreatePluginAccountInput {
            plugin_id: "google-workspace".to_string(),
            account_identifier: "alice.personal@gmail.com".to_string(),
            label: "Gmail Personnel".to_string(),
            email: Some("alice.personal@gmail.com".to_string()),
            display_name: Some("Alice Dupont".to_string()),
            avatar_url: Some("https://example.com/alice.jpg".to_string()),
            auth_method: "oauth2".to_string(),
            is_default: None,
            status: Some("active".to_string()),
        })
        .unwrap();

    assert_eq!(google_personal.plugin_id, "google-workspace");
    assert_eq!(google_personal.label, "Gmail Personnel");
    assert!(google_personal.is_default);

    // 2. Add second Google account (Work) -> should NOT be default unless specified
    let google_work = manager
        .create_account(CreatePluginAccountInput {
            plugin_id: "google-workspace".to_string(),
            account_identifier: "alice@acme-corp.com".to_string(),
            label: "Gmail Entreprise".to_string(),
            email: Some("alice@acme-corp.com".to_string()),
            display_name: Some("Alice Acme".to_string()),
            avatar_url: None,
            auth_method: "oauth2".to_string(),
            is_default: Some(false),
            status: Some("active".to_string()),
        })
        .unwrap();

    assert_eq!(google_work.label, "Gmail Entreprise");
    assert!(!google_work.is_default);

    // 3. Add GitHub account (PAT) -> separate plugin_id, should be default for GitHub
    let github_main = manager
        .create_account(CreatePluginAccountInput {
            plugin_id: "github-developer".to_string(),
            account_identifier: "alice-coder".to_string(),
            label: "GitHub Principal".to_string(),
            email: Some("alice.personal@gmail.com".to_string()),
            display_name: Some("Alice Coder".to_string()),
            avatar_url: None,
            auth_method: "pat".to_string(),
            is_default: None,
            status: Some("active".to_string()),
        })
        .unwrap();

    assert!(github_main.is_default);

    // 4. Verify list filtering
    let google_list = manager.list_accounts(Some("google-workspace")).unwrap();
    assert_eq!(google_list.len(), 2);
    assert_eq!(google_list[0].id, google_personal.id); // default first

    let github_list = manager.list_accounts(Some("github-developer")).unwrap();
    assert_eq!(github_list.len(), 1);

    let all_list = manager.list_accounts(None).unwrap();
    assert_eq!(all_list.len(), 3);

    // 5. Switch default account for Google to Work
    let updated_work = manager.set_default_account(&google_work.id).unwrap();
    assert!(updated_work.is_default);

    let re_fetched_personal = manager.get_account(&google_personal.id).unwrap().unwrap();
    assert!(!re_fetched_personal.is_default, "Previous default must be false");

    // 6. Update label
    let relabeled = manager
        .update_account_label(&google_personal.id, "Gmail Perso (Ancien)")
        .unwrap();
    assert_eq!(relabeled.label, "Gmail Perso (Ancien)");

    // 7. Update status
    let updated_status = manager
        .update_account_status(&google_work.id, "expired")
        .unwrap();
    assert_eq!(updated_status.status, "expired");

    // 8. Delete active default (google_work) -> google_personal should automatically become default
    manager.delete_account(&google_work.id).unwrap();
    let remaining_google = manager.list_accounts(Some("google-workspace")).unwrap();
    assert_eq!(remaining_google.len(), 1);
    assert!(remaining_google[0].is_default);
    assert_eq!(remaining_google[0].id, google_personal.id);
}
