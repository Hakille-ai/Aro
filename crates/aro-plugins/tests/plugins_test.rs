use aro_plugins::{
    get_curated_marketplace, McpManifest, PluginManager, PluginManifest, PluginSourceType,
    PluginStatus, CANONICAL_PLUGIN_SCHEMA_V1,
};
use aro_skills::SkillRegistry;
use std::fs;
use std::sync::Arc;
use tempfile::tempdir;

#[test]
fn test_plugin_manifest_validation() {
    let valid_json = format!(
        r#"{{
            "$schema": "{CANONICAL_PLUGIN_SCHEMA_V1}",
            "name": "my-cool-agent-plugin",
            "version": "1.0.0",
            "description": "A test plugin following agent-plugins.org",
            "author": {{
                "name": "ARO Team",
                "email": "dev@aro.local"
            }},
            "keywords": ["ai", "agents", "test"]
        }}"#
    );

    let manifest = PluginManifest::from_json_str(&valid_json).expect("valid manifest");
    assert_eq!(manifest.name, "my-cool-agent-plugin");
    assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
    assert_eq!(manifest.keywords.len(), 3);
}

#[test]
fn test_plugin_manifest_ignores_unknown_fields() {
    // Unknown properties in plugin.json are reported and ignored per agent-plugins.org spec §5.2
    let json = format!(
        r#"{{
            "$schema": "{CANONICAL_PLUGIN_SCHEMA_V1}",
            "name": "plugin-with-extra",
            "mcpServers": {{
                "srv": {{ "type": "stdio", "command": "run" }}
            }}
        }}"#
    );

    let manifest =
        PluginManifest::from_json_str(&json).expect("should load manifest ignoring unknown fields");
    assert_eq!(manifest.name, "plugin-with-extra");
}

#[test]
fn test_plugin_manifest_rejects_invalid_names() {
    let invalid_names = [
        "MyPlugin",
        "my_plugin",
        "-my-plugin",
        "my-plugin-",
        "my--plugin",
        "my..plugin",
        "plugin!",
    ];
    for bad_name in invalid_names {
        let json = format!(
            r#"{{
                "$schema": "{CANONICAL_PLUGIN_SCHEMA_V1}",
                "name": "{bad_name}"
            }}"#
        );
        assert!(
            PluginManifest::from_json_str(&json).is_err(),
            "should reject '{bad_name}'"
        );
    }

    // Valid names with dots per §5.5
    let valid_dot_name = format!(
        r#"{{
            "$schema": "{CANONICAL_PLUGIN_SCHEMA_V1}",
            "name": "company.tool-v2"
        }}"#
    );
    assert!(PluginManifest::from_json_str(&valid_dot_name).is_ok());
}

#[test]
fn test_mcp_config_resolution() {
    let mcp_json = r#"{
        "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
        "mcpServers": {
            "test-srv": {
                "type": "stdio",
                "command": "python",
                "args": ["${PLUGIN_ROOT}/server.py", "--cache-dir", "${PLUGIN_DATA}/cache"],
                "env": {
                    "DATA_DIR": "${PLUGIN_DATA}/store"
                },
                "cwd": "${PLUGIN_ROOT}"
            }
        }
    }"#;

    let mcp_manifest = McpManifest::from_json_str(mcp_json).expect("valid mcp.json");
    let temp = tempdir().unwrap();
    let root = temp.path().join("plugin-root");
    let data = temp.path().join("plugin-data");

    let resolved = mcp_manifest
        .resolve_server_config("test-srv", &root, &data)
        .unwrap();
    assert_eq!(resolved.command.as_deref(), Some("python"));
    assert_eq!(resolved.args[0], format!("{}/server.py", root.display()));
    assert_eq!(resolved.args[2], format!("{}/cache", data.display()));
    assert_eq!(
        resolved.env.get("DATA_DIR").unwrap(),
        &format!("{}/store", data.display())
    );
    assert_eq!(
        resolved.env.get("PLUGIN_ROOT").unwrap(),
        &root.display().to_string()
    );
    assert_eq!(
        resolved.env.get("PLUGIN_DATA").unwrap(),
        &data.display().to_string()
    );
}

#[test]
fn test_mcp_config_rejects_plugin_root_in_env() {
    let bad_mcp_json = r#"{
        "mcpServers": {
            "bad-srv": {
                "type": "stdio",
                "command": "node",
                "env": {
                    "PLUGIN_ROOT": "/illegal/override"
                }
            }
        }
    }"#;

    assert!(McpManifest::from_json_str(bad_mcp_json).is_err());
}

#[tokio::test]
async fn test_plugin_manager_install_marketplace_and_lifecycle() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // 1. Install from curated marketplace
    let installed = manager
        .install_from_marketplace("filesystem-tools")
        .await
        .expect("marketplace install success");

    assert_eq!(installed.id, "filesystem-tools");
    assert_eq!(installed.mcp_servers.len(), 1);
    assert_eq!(installed.mcp_servers[0].name, "local-fs");
    assert_eq!(installed.skills.len(), 1);
    assert_eq!(installed.skills[0].id, "workspace-explorer");

    // Check that skill was registered in skill registry
    let skill = registry.get("workspace-explorer").await;
    assert!(skill.is_some());

    // 2. Toggle enabled state
    let disabled = manager
        .set_enabled("filesystem-tools", false)
        .await
        .unwrap();
    assert!(!disabled.enabled);

    let reenabled = manager.set_enabled("filesystem-tools", true).await.unwrap();
    assert!(reenabled.enabled);

    // 3. Uninstall
    manager
        .uninstall("filesystem-tools")
        .await
        .expect("uninstall success");
    assert!(manager.get_plugin("filesystem-tools").await.is_none());
    assert!(registry.get("workspace-explorer").await.is_none());
}

#[tokio::test]
async fn test_skill_discovery_failure_isolation() {
    let temp = tempdir().unwrap();
    let plugin_dir = temp.path().join("faulty-plugin");
    fs::create_dir_all(plugin_dir.join("skills").join("good-skill")).unwrap();
    fs::create_dir_all(plugin_dir.join("skills").join("broken-skill")).unwrap();

    // Valid skill
    fs::write(
        plugin_dir
            .join("skills")
            .join("good-skill")
            .join("SKILL.md"),
        "---\nname: Good Skill\ndescription: Works fine\n---\n# Instructions\nDo good things.\n",
    )
    .unwrap();

    // Broken skill without frontmatter end fence
    fs::write(
        plugin_dir
            .join("skills")
            .join("broken-skill")
            .join("SKILL.md"),
        "---\nname: Broken Skill without closing fence",
    )
    .unwrap();

    // Plugin manifest
    fs::write(
        plugin_dir.join("plugin.json"),
        r#"{
            "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
            "name": "faulty-plugin"
        }"#,
    )
    .unwrap();

    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().join("base"), registry.clone());

    // Plugin installation should succeed due to failure isolation on broken skill
    let installed = manager
        .install_from_directory(&plugin_dir)
        .await
        .expect("should succeed with failure isolation");

    assert_eq!(installed.skills.len(), 1);
    assert_eq!(installed.skills[0].id, "good-skill");
}

#[tokio::test]
async fn test_plugin_mcp_real_execution_and_skills() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // 1. Install filesystem-tools
    let installed = manager
        .install_from_marketplace("filesystem-tools")
        .await
        .expect("marketplace install");

    assert_eq!(installed.id, "filesystem-tools");

    // 2. Test MCP server tool listing
    let tools = manager
        .test_mcp_server("filesystem-tools", "local-fs")
        .await
        .expect("test mcp server tools");

    assert!(tools.iter().any(|t| t.name == "read_file"));
    assert!(tools.iter().any(|t| t.name == "write_file"));
    assert!(tools.iter().any(|t| t.name == "list_dir"));

    // 3. Write a real file via MCP tool
    let test_file = temp.path().join("mcp_test_doc.txt");
    let test_content = "Hello ARO AGI Workspace!";
    let write_res = manager
        .call_mcp_tool(
            "filesystem-tools",
            "local-fs",
            "write_file",
            serde_json::json!({
                "path": test_file.to_string_lossy().to_string(),
                "content": test_content
            }),
        )
        .await
        .expect("call write_file");

    assert!(!write_res.content.is_empty());

    // 4. Read back the file via MCP tool
    let read_res = manager
        .call_mcp_tool(
            "filesystem-tools",
            "local-fs",
            "read_file",
            serde_json::json!({
                "path": test_file.to_string_lossy().to_string()
            }),
        )
        .await
        .expect("call read_file");

    assert_eq!(read_res.plain_text().trim(), test_content);

    // 5. Invoke the installed skill
    let skill_output = manager
        .invoke_skill(
            "filesystem-tools",
            "workspace-explorer",
            &serde_json::json!({ "path": temp.path().to_string_lossy().to_string() }),
        )
        .await
        .expect("invoke workspace-explorer");

    assert!(skill_output.success);
    assert!(!skill_output.output.is_empty());

    // 6. Test Python Analytics MCP tool
    let _analytics = manager
        .install_from_marketplace("python-analytics")
        .await
        .expect("marketplace install python-analytics");

    let stats_res = manager
        .call_mcp_tool(
            "python-analytics",
            "python-kernel",
            "compute_stats",
            serde_json::json!({
                "numbers": [10.0, 20.0, 30.0, 40.0, 50.0]
            }),
        )
        .await
        .expect("call compute_stats");

    assert!(
        stats_res.plain_text().contains("\"mean\": 30.0") || stats_res.plain_text().contains("30")
    );
}

#[tokio::test]
async fn test_sqlite_mcp_real_queries_and_code_reviewer() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // 1. Install sqlite-database
    manager
        .install_from_marketplace("sqlite-database")
        .await
        .expect("marketplace install sqlite-database");

    let db_file = temp.path().join("test_app.sqlite");

    // 2. Create table
    let create_res = manager
        .call_mcp_tool(
            "sqlite-database",
            "sqlite-server",
            "execute_query",
            serde_json::json!({
                "db_path": db_file.to_string_lossy().to_string(),
                "query": "CREATE TABLE employees (id INTEGER PRIMARY KEY, name TEXT, salary REAL);"
            }),
        )
        .await
        .expect("create table");
    assert_eq!(create_res.is_error, Some(false));

    // 3. Insert rows
    manager
        .call_mcp_tool(
            "sqlite-database",
            "sqlite-server",
            "execute_query",
            serde_json::json!({
                "db_path": db_file.to_string_lossy().to_string(),
                "query": "INSERT INTO employees (name, salary) VALUES ('Ada Lovelace', 120000.0), ('Alan Turing', 130000.0);"
            }),
        )
        .await
        .expect("insert rows");

    // 4. Query rows
    let select_res = manager
        .call_mcp_tool(
            "sqlite-database",
            "sqlite-server",
            "execute_query",
            serde_json::json!({
                "db_path": db_file.to_string_lossy().to_string(),
                "query": "SELECT name, salary FROM employees ORDER BY salary DESC;"
            }),
        )
        .await
        .expect("select rows");

    let select_text = select_res.plain_text();
    assert!(select_text.contains("Ada Lovelace"));
    assert!(select_text.contains("Alan Turing"));
    assert!(select_text.contains("130000"));

    // 5. List tables
    let tables_res = manager
        .call_mcp_tool(
            "sqlite-database",
            "sqlite-server",
            "list_tables",
            serde_json::json!({
                "db_path": db_file.to_string_lossy().to_string()
            }),
        )
        .await
        .expect("list tables");
    assert!(tables_res.plain_text().contains("employees"));

    // 6. Schema info
    let schema_res = manager
        .call_mcp_tool(
            "sqlite-database",
            "sqlite-server",
            "schema_info",
            serde_json::json!({
                "db_path": db_file.to_string_lossy().to_string(),
                "table": "employees"
            }),
        )
        .await
        .expect("schema info");
    assert!(schema_res.plain_text().contains("salary"));

    // 7. Install code-reviewer and test lint_check
    manager
        .install_from_marketplace("code-reviewer")
        .await
        .expect("marketplace install code-reviewer");

    let broken_code = "fn bad_syntax( { let x = 10;";
    let lint_broken = manager
        .call_mcp_tool(
            "code-reviewer",
            "code-reviewer-server",
            "lint_check",
            serde_json::json!({ "code": broken_code }),
        )
        .await
        .expect("lint check broken");
    assert_eq!(lint_broken.is_error, Some(true));
    assert!(
        lint_broken.plain_text().to_lowercase().contains("unclosed")
            || lint_broken.plain_text().contains("warning")
    );

    let clean_code = "fn good_syntax() { let x = 10; }\n";
    let lint_clean = manager
        .call_mcp_tool(
            "code-reviewer",
            "code-reviewer-server",
            "lint_check",
            serde_json::json!({ "code": clean_code }),
        )
        .await
        .expect("lint check clean");
    assert_eq!(lint_clean.is_error, Some(false));
    assert!(lint_clean.plain_text().contains("clean"));
}

#[tokio::test]
async fn test_load_all_seeds_default_plugins() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // Initially installed list is empty
    assert_eq!(manager.list_installed().await.len(), 0);

    // Calling load_all on empty directory should trigger ensure_default_plugins
    manager.load_all().await.expect("load_all succeeds");

    let installed = manager.list_installed().await;
    assert_eq!(installed.len(), 5);

    let expected_ids = [
        "filesystem-tools",
        "web-search-tools",
        "git-assistant",
        "sqlite-database",
        "python-analytics",
    ];

    for id in &expected_ids {
        let plugin = installed.iter().find(|p| p.id == *id);
        assert!(plugin.is_some(), "Default plugin {} should be installed", id);
        let p = plugin.unwrap();
        assert!(p.is_system, "Default plugin {} must have is_system = true", id);
        assert!(p.enabled, "Default plugin {} should be enabled", id);
        assert!(!p.skills.is_empty(), "Default plugin {} must have skills", id);
    }

    // Verify marketplace listing shows default plugins as installed
    let market = manager.list_marketplace().await;
    for id in &expected_ids {
        let item = market.iter().find(|m| m.id == *id).unwrap();
        assert!(item.installed, "Marketplace item {} must be marked installed", id);
    }

    // Non-default plugins should NOT be marked installed
    let non_default = ["google-workspace", "openai-ecosystem", "github-developer", "slack-workspace", "code-reviewer"];
    for id in &non_default {
        let item = market.iter().find(|m| m.id == *id).unwrap();
        assert!(!item.installed, "Plugin {} should not be installed yet", id);
    }
}

#[tokio::test]
async fn test_ecosystem_plugins_installation_and_tools() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // 1. Install google-workspace
    let g_plugin = manager
        .install_from_marketplace("google-workspace")
        .await
        .expect("install google-workspace");
    assert_eq!(g_plugin.id, "google-workspace");
    assert!(!g_plugin.is_system, "marketplace plugin must not be system");
    assert_eq!(g_plugin.mcp_servers.len(), 1);
    assert!(g_plugin.skills.iter().any(|s| s.id == "workspace-organizer"));

    let cal_res = manager
        .call_mcp_tool(
            "google-workspace",
            "google-workspace-mcp",
            "list_calendar_events",
            serde_json::json!({ "days_ahead": 14 }),
        )
        .await
        .expect("call list_calendar_events");
    assert!(cal_res.plain_text().contains("Sprint Planning"));

    // 2. Install openai-ecosystem
    let o_plugin = manager
        .install_from_marketplace("openai-ecosystem")
        .await
        .expect("install openai-ecosystem");
    assert_eq!(o_plugin.id, "openai-ecosystem");
    assert!(!o_plugin.is_system);

    let token_res = manager
        .call_mcp_tool(
            "openai-ecosystem",
            "openai-mcp",
            "token_counter",
            serde_json::json!({ "text": "Hello world from ARO agent plugins!" }),
        )
        .await
        .expect("call token_counter");
    assert!(token_res.plain_text().contains("estimated_tokens"));

    let models_res = manager
        .call_mcp_tool(
            "openai-ecosystem",
            "openai-mcp",
            "list_models",
            serde_json::json!({}),
        )
        .await
        .expect("call list_models");
    assert!(models_res.plain_text().contains("gpt-4o"));

    // 3. Install github-developer
    let gh_plugin = manager
        .install_from_marketplace("github-developer")
        .await
        .expect("install github-developer");
    assert_eq!(gh_plugin.id, "github-developer");

    let issues_res = manager
        .call_mcp_tool(
            "github-developer",
            "github-mcp",
            "list_issues",
            serde_json::json!({ "repo": "aro/aro" }),
        )
        .await
        .expect("call list_issues");
    assert!(issues_res.plain_text().contains("Support default agent plugins"));

    // 4. Install slack-workspace
    let sl_plugin = manager
        .install_from_marketplace("slack-workspace")
        .await
        .expect("install slack-workspace");
    assert_eq!(sl_plugin.id, "slack-workspace");

    let msg_res = manager
        .call_mcp_tool(
            "slack-workspace",
            "slack-mcp",
            "post_message",
            serde_json::json!({ "channel": "dev-channel", "text": "Default plugins working!" }),
        )
        .await
        .expect("call post_message");
    assert!(msg_res.plain_text().contains("ok"));
}

#[tokio::test]
async fn test_disabled_plugin_state_and_skill_unregistration_persists_across_reloads() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // Install google-workspace
    let p = manager
        .install_from_marketplace("google-workspace")
        .await
        .expect("install google-workspace");
    assert!(p.enabled);
    assert_eq!(p.status, PluginStatus::Active);

    // Skill should be registered
    assert!(registry.get("workspace-organizer").await.is_some());

    // Disable plugin
    let disabled = manager
        .set_enabled("google-workspace", false)
        .await
        .expect("disable plugin");
    assert!(!disabled.enabled);
    assert_eq!(disabled.status, PluginStatus::Inactive);

    // Skill should now be unregistered
    assert!(registry.get("workspace-organizer").await.is_none());

    // Simulate app restart with a fresh PluginManager on the same directory
    let registry2 = Arc::new(SkillRegistry::new());
    let manager2 = PluginManager::new(temp.path().to_path_buf(), registry2.clone());
    manager2.load_all().await.expect("load_all succeeds");

    let loaded = manager2
        .get_plugin("google-workspace")
        .await
        .expect("plugin exists after reload");
    assert!(!loaded.enabled, "Plugin must remain disabled across app restarts");
    assert_eq!(loaded.status, PluginStatus::Inactive);
    assert!(
        registry2.get("workspace-organizer").await.is_none(),
        "Disabled plugin skills must not be registered on reload"
    );

    // Re-enable plugin
    let re_enabled = manager2
        .set_enabled("google-workspace", true)
        .await
        .expect("re-enable plugin");
    assert!(re_enabled.enabled);
    assert_eq!(re_enabled.status, PluginStatus::Active);
    assert!(
        registry2.get("workspace-organizer").await.is_some(),
        "Re-enabled plugin skills must be registered in registry"
    );

    // Simulate second app restart
    let registry3 = Arc::new(SkillRegistry::new());
    let manager3 = PluginManager::new(temp.path().to_path_buf(), registry3.clone());
    manager3.load_all().await.expect("load_all succeeds");

    let loaded3 = manager3
        .get_plugin("google-workspace")
        .await
        .expect("plugin exists");
    assert!(loaded3.enabled, "Plugin must remain enabled across app restarts");
    assert_eq!(loaded3.status, PluginStatus::Active);
    assert!(
        registry3.get("workspace-organizer").await.is_some(),
        "Enabled plugin skills must be registered on reload"
    );
}

#[tokio::test]
async fn test_uninstalled_plugins_not_resurrected() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    // Initial load seeds default plugins
    manager.load_all().await.expect("initial seed");
    assert_eq!(manager.list_installed().await.len(), 5);

    // User uninstalls all plugins
    for id in &[
        "filesystem-tools",
        "web-search-tools",
        "git-assistant",
        "sqlite-database",
        "python-analytics",
    ] {
        manager.uninstall(id).await.expect("uninstall succeeds");
    }

    assert_eq!(manager.list_installed().await.len(), 0);

    // Simulate app restart: fresh manager on same directory
    let registry2 = Arc::new(SkillRegistry::new());
    let manager2 = PluginManager::new(temp.path().to_path_buf(), registry2.clone());
    manager2.load_all().await.expect("subsequent load_all");

    // Intentionally uninstalled plugins MUST NOT be resurrected!
    assert_eq!(
        manager2.list_installed().await.len(),
        0,
        "User uninstalled plugins must not be resurrected on next app start"
    );
}

#[tokio::test]
async fn test_marketplace_source_and_installed_at_persists_across_reloads() {
    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    let installed = manager
        .install_from_marketplace("openai-ecosystem")
        .await
        .expect("install");
    assert_eq!(installed.source, PluginSourceType::Marketplace);
    let original_installed_at = installed.installed_at;

    // Simulate app restart
    let registry2 = Arc::new(SkillRegistry::new());
    let manager2 = PluginManager::new(temp.path().to_path_buf(), registry2.clone());
    manager2.load_all().await.expect("load_all");

    let reloaded = manager2
        .get_plugin("openai-ecosystem")
        .await
        .expect("plugin found");
    assert_eq!(
        reloaded.source,
        PluginSourceType::Marketplace,
        "PluginSourceType::Marketplace must be preserved and not overwritten as Local"
    );
    assert_eq!(
        reloaded.installed_at, original_installed_at,
        "installed_at must be preserved across app restarts"
    );
}

#[tokio::test]
async fn test_native_mcp_fallback_for_ecosystem_tools() {
    // 1. google-workspace-mcp
    let g_tools = PluginManager::native_mcp_tools("google-workspace-mcp");
    assert_eq!(g_tools.len(), 4);
    assert!(g_tools.iter().any(|t| t.name == "search_drive"));
    assert!(g_tools.iter().any(|t| t.name == "read_doc"));
    assert!(g_tools.iter().any(|t| t.name == "list_calendar_events"));
    assert!(g_tools.iter().any(|t| t.name == "draft_email"));

    // 2. openai-mcp
    let o_tools = PluginManager::native_mcp_tools("openai-mcp");
    assert_eq!(o_tools.len(), 4);
    assert!(o_tools.iter().any(|t| t.name == "list_models"));
    assert!(o_tools.iter().any(|t| t.name == "token_counter"));
    assert!(o_tools.iter().any(|t| t.name == "format_completion"));
    assert!(o_tools.iter().any(|t| t.name == "validate_schema"));

    // 3. github-mcp
    let gh_tools = PluginManager::native_mcp_tools("github-mcp");
    assert_eq!(gh_tools.len(), 4);
    assert!(gh_tools.iter().any(|t| t.name == "list_issues"));
    assert!(gh_tools.iter().any(|t| t.name == "create_issue"));
    assert!(gh_tools.iter().any(|t| t.name == "get_pull_request"));
    assert!(gh_tools.iter().any(|t| t.name == "list_actions"));

    // 4. slack-mcp
    let sl_tools = PluginManager::native_mcp_tools("slack-mcp");
    assert_eq!(sl_tools.len(), 3);
    assert!(sl_tools.iter().any(|t| t.name == "post_message"));
    assert!(sl_tools.iter().any(|t| t.name == "list_channels"));
    assert!(sl_tools.iter().any(|t| t.name == "read_history"));
}

#[tokio::test]
async fn test_code_reviewer_marketplace_metadata_matches_scaffold() {
    let marketplace = get_curated_marketplace();
    let cr_item = marketplace
        .iter()
        .find(|m| m.id == "code-reviewer")
        .expect("code-reviewer in marketplace");
    assert_eq!(
        cr_item.mcp_servers_count, 1,
        "code-reviewer has 1 MCP server in scaffold"
    );
    assert_eq!(
        cr_item.sample_mcp_servers,
        vec!["code-reviewer-server".to_string()]
    );

    let temp = tempdir().unwrap();
    let registry = Arc::new(SkillRegistry::new());
    let manager = PluginManager::new(temp.path().to_path_buf(), registry.clone());

    let installed = manager
        .install_from_marketplace("code-reviewer")
        .await
        .expect("install code-reviewer");
    assert_eq!(installed.mcp_servers.len(), 1);
    assert_eq!(installed.mcp_servers[0].name, "code-reviewer-server");
}

