use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use serde_json::Value;
use tokio::sync::RwLock;

use aro_mcp::{CallToolResult, McpClient, McpContent, McpTool};
use aro_skills::{SkillOutput, SkillRegistry};

use crate::accounts::{CreatePluginAccountInput, PluginAccount, PluginAccountStore};
use crate::manifest::PluginManifest;
use crate::marketplace::{get_curated_marketplace, MarketplacePlugin};
use crate::mcp_config::{McpManifest, McpTransportType};
use crate::model::{
    InstalledPlugin, PluginMcpServerSummary, PluginSkillSummary, PluginSourceType, PluginStatus,
};
use crate::skills_discovery::discover_plugin_skills;

pub const CORE_DEFAULT_PLUGINS: &[&str] = &[
    "filesystem-tools",
    "web-search-tools",
    "git-assistant",
    "sqlite-database",
    "python-analytics",
];

pub struct PluginManager {
    base_dir: PathBuf,
    installed_dir: PathBuf,
    data_dir: PathBuf,
    plugins: Arc<RwLock<HashMap<String, InstalledPlugin>>>,
    mcp_manifests: Arc<RwLock<HashMap<String, McpManifest>>>,
    skill_registry: Arc<SkillRegistry>,
    initialized: Arc<AtomicBool>,
    account_store: Arc<PluginAccountStore>,
}

impl PluginManager {
    pub fn new(base_dir: PathBuf, skill_registry: Arc<SkillRegistry>) -> Self {
        let installed_dir = base_dir.join("installed");
        let data_dir = base_dir.join("data");

        let _ = std::fs::create_dir_all(&installed_dir);
        let _ = std::fs::create_dir_all(&data_dir);

        let accounts_db = data_dir.join("accounts.sqlite3");
        let account_store = Arc::new(
            PluginAccountStore::new(accounts_db).expect("plugin accounts store initialization"),
        );

        Self {
            base_dir,
            installed_dir,
            data_dir,
            plugins: Arc::new(RwLock::new(HashMap::new())),
            mcp_manifests: Arc::new(RwLock::new(HashMap::new())),
            skill_registry,
            initialized: Arc::new(AtomicBool::new(false)),
            account_store,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Relaxed)
    }

    pub fn skill_registry(&self) -> Arc<SkillRegistry> {
        self.skill_registry.clone()
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn account_store(&self) -> &PluginAccountStore {
        &self.account_store
    }

    pub fn list_accounts(&self, plugin_id: Option<&str>) -> Result<Vec<PluginAccount>> {
        self.account_store.list(plugin_id)
    }

    pub fn get_account(&self, id: &str) -> Result<Option<PluginAccount>> {
        self.account_store.get(id)
    }

    pub fn create_account(&self, input: CreatePluginAccountInput) -> Result<PluginAccount> {
        self.account_store.create(input)
    }

    pub fn set_default_account(&self, id: &str) -> Result<PluginAccount> {
        self.account_store.set_default(id)
    }

    pub fn update_account_label(&self, id: &str, label: &str) -> Result<PluginAccount> {
        self.account_store.update_label(id, label)
    }

    pub fn update_account_status(&self, id: &str, status: &str) -> Result<PluginAccount> {
        self.account_store.update_status(id, status)
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        self.account_store.delete(id)
    }

    pub async fn load_all(&self) -> Result<()> {
        self.initialized.store(true, Ordering::Relaxed);

        if !self.installed_dir.is_dir() {
            let _ = std::fs::create_dir_all(&self.installed_dir);
        }

        let entries = std::fs::read_dir(&self.installed_dir)
            .with_context(|| format!("failed to read {}", self.installed_dir.display()))?;

        let mut loaded_count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("plugin.json").is_file() {
                match self.load_plugin_dir(&path, PluginSourceType::Local).await {
                    Ok(_) => {
                        loaded_count += 1;
                    }
                    Err(err) => {
                        tracing::warn!(?err, path = %path.display(), "failed to load installed plugin");
                    }
                }
            }
        }

        let seeded_marker = self.base_dir.join(".seeded");
        if loaded_count == 0 && !seeded_marker.exists() {
            self.ensure_default_plugins().await?;
        }

        Ok(())
    }

    pub async fn ensure_default_plugins(&self) -> Result<()> {
        let _ = std::fs::create_dir_all(&self.installed_dir);

        for plugin_id in CORE_DEFAULT_PLUGINS {
            let target_dir = self.installed_dir.join(plugin_id);
            let manifest_path = target_dir.join("plugin.json");

            if !manifest_path.is_file() {
                if target_dir.exists() {
                    let _ = std::fs::remove_dir_all(&target_dir);
                }
                std::fs::create_dir_all(&target_dir)?;

                if let Err(err) =
                    crate::scaffold::scaffold_marketplace_plugin(&target_dir, plugin_id)
                {
                    tracing::warn!(?err, plugin_id, "failed to scaffold default plugin");
                    continue;
                }

                let _ = std::fs::write(target_dir.join(".system"), "true\n");
                let _ = std::fs::write(target_dir.join(".source"), "marketplace\n");
            }

            if let Err(err) = self
                .load_plugin_dir(&target_dir, PluginSourceType::Marketplace)
                .await
            {
                tracing::warn!(?err, plugin_id, "failed to load default plugin");
            }
        }

        let _ = std::fs::write(self.base_dir.join(".seeded"), "true\n");
        Ok(())
    }

    pub async fn load_plugin_dir(
        &self,
        plugin_root: &Path,
        source: PluginSourceType,
    ) -> Result<InstalledPlugin> {
        let manifest_file = plugin_root.join("plugin.json");
        let content = std::fs::read_to_string(&manifest_file)
            .with_context(|| format!("missing plugin.json in {}", plugin_root.display()))?;

        let manifest = PluginManifest::from_json_str(&content).with_context(|| {
            format!(
                "failed to validate plugin.json in {}",
                plugin_root.display()
            )
        })?;

        let plugin_id = manifest.name.clone();
        let plugin_data = self.data_dir.join(&plugin_id);
        let _ = std::fs::create_dir_all(&plugin_data);

        // Load mcp.json if present
        let mcp_file = plugin_root.join("mcp.json");
        let mut mcp_summaries = Vec::new();
        if mcp_file.is_file() {
            if let Ok(mcp_content) = std::fs::read_to_string(&mcp_file) {
                match McpManifest::from_json_str(&mcp_content) {
                    Ok(mcp_manifest) => {
                        for (name, srv) in &mcp_manifest.mcp_servers {
                            let (transport_str, cmd, url) = match srv.transport_type {
                                McpTransportType::Stdio => {
                                    ("stdio".to_string(), srv.command.clone(), None)
                                }
                                McpTransportType::StreamableHttp => {
                                    ("streamable-http".to_string(), None, srv.url.clone())
                                }
                                McpTransportType::Sse => ("sse".to_string(), None, srv.url.clone()),
                            };

                            mcp_summaries.push(PluginMcpServerSummary {
                                name: name.clone(),
                                transport_type: transport_str,
                                command: cmd,
                                url,
                                status: "ready".to_string(),
                            });
                        }
                        let mut mcp_map = self.mcp_manifests.write().await;
                        mcp_map.insert(plugin_id.clone(), mcp_manifest);
                    }
                    Err(err) => {
                        tracing::warn!(?err, "invalid mcp.json in {}", plugin_root.display());
                    }
                }
            }
        }

        // Discover skills
        let discovered_skills = discover_plugin_skills(plugin_root);
        let disabled_file = plugin_root.join(".disabled");
        let enabled = !disabled_file.is_file();
        let status = if enabled {
            PluginStatus::Active
        } else {
            PluginStatus::Inactive
        };

        let mut skill_summaries = Vec::new();
        for s in &discovered_skills {
            if enabled {
                self.skill_registry.register(s.skill.clone()).await;
            }
            skill_summaries.push(PluginSkillSummary {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                icon: s.icon.clone(),
                tags: s.tags.clone(),
                has_scripts: !s.skill.scripts.is_empty(),
            });
        }

        let is_system = CORE_DEFAULT_PLUGINS.contains(&plugin_id.as_str())
            || plugin_root.join(".system").is_file();

        let source = if plugin_root.join(".source").is_file() {
            match std::fs::read_to_string(plugin_root.join(".source"))
                .unwrap_or_default()
                .trim()
            {
                "marketplace" => PluginSourceType::Marketplace,
                "git" => PluginSourceType::Git,
                "local" => PluginSourceType::Local,
                _ => source,
            }
        } else {
            source
        };

        let installed_at_path = plugin_root.join(".installed_at");
        let installed_at = if installed_at_path.is_file() {
            if let Ok(ts_str) = std::fs::read_to_string(&installed_at_path) {
                chrono::DateTime::parse_from_rfc3339(ts_str.trim())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now())
            } else {
                Utc::now()
            }
        } else {
            let now = Utc::now();
            let _ = std::fs::write(&installed_at_path, now.to_rfc3339());
            now
        };

        let branding = crate::branding::branding_from_extensions(&manifest.extensions);
        let installed = InstalledPlugin {
            id: plugin_id.clone(),
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            author: manifest
                .author
                .as_ref()
                .map(|a| a.display_name().to_string()),
            homepage: manifest.homepage.clone(),
            repository: manifest.repository.clone(),
            license: manifest.license.clone(),
            keywords: manifest.keywords.clone(),
            root_path: plugin_root.display().to_string(),
            data_path: plugin_data.display().to_string(),
            enabled,
            is_system,
            status,
            status_message: None,
            installed_at,
            source,
            mcp_servers: mcp_summaries,
            skills: skill_summaries,
            auth: get_curated_marketplace()
                .into_iter()
                .find(|m| m.id == plugin_id)
                .and_then(|m| m.auth),
            extensions: manifest.extensions.clone(),
            logo: branding.logo,
            logo_kind: branding.logo_kind.map(|k| match k {
                crate::branding::LogoKind::Emoji => "emoji".to_string(),
                crate::branding::LogoKind::File => "file".to_string(),
            }),
            brand_color: branding.brand_color,
        };

        let mut map = self.plugins.write().await;
        map.insert(plugin_id, installed.clone());
        Ok(installed)
    }

    pub async fn install_from_directory(&self, source_dir: &Path) -> Result<InstalledPlugin> {
        let manifest_path = source_dir.join("plugin.json");
        if !manifest_path.is_file() {
            return Err(anyhow!("No plugin.json found in {}", source_dir.display()));
        }

        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest = PluginManifest::from_json_str(&content)?;

        let target_dir = self.installed_dir.join(&manifest.name);
        if target_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_dir);
        }

        // Copy directory recursively
        copy_dir_all(source_dir, &target_dir)?;
        let _ = std::fs::write(target_dir.join(".source"), "local\n");

        self.load_plugin_dir(&target_dir, PluginSourceType::Local)
            .await
    }

    pub async fn install_from_git(&self, git_url: &str) -> Result<InstalledPlugin> {
        let url = validate_git_url(git_url)?;
        let temp_dir = fresh_clone_dir()?;
        // Le dossier temporaire est TOUJOURS nettoyé, succès comme échec.
        let result = self.install_from_git_inner(&url, &temp_dir).await;
        let _ = std::fs::remove_dir_all(&temp_dir);
        result
    }

    async fn install_from_git_inner(
        &self,
        git_url: &str,
        temp_dir: &Path,
    ) -> Result<InstalledPlugin> {
        let dir_str = temp_dir.to_string_lossy().to_string();

        // 1. Sonde : clone sparse + shallow, seuls les fichiers racine sont
        //    matérialisés. Rapide, et insensible aux chemins profonds du dépôt
        //    (~5000 fichiers non téléchargés pour valider un seul manifeste).
        let mut probe_args: Vec<String> = vec![
            "-c".into(),
            "core.longpaths=true".into(),
            "clone".into(),
            "--depth".into(),
            "1".into(),
            "--filter=blob:none".into(),
            "--sparse".into(),
            git_url.into(),
            dir_str.clone(),
        ];
        let mut output = run_git(&probe_args, GIT_PROBE_TIMEOUT)
            .await
            .map_err(|err| classify_spawn_failure(&err.to_string()))?;
        if !output.status.success() && mentions_filter_unsupported(&output.stderr) {
            // Git ou serveur trop ancien pour --filter : on réessaie sans,
            // toujours en sparse + shallow.
            probe_args.remove(6);
            output = run_git(&probe_args, GIT_PROBE_TIMEOUT)
                .await
                .map_err(|err| classify_spawn_failure(&err.to_string()))?;
        }
        if !output.status.success() {
            return Err(classify_git_failure("clone", &output.stderr));
        }

        // 2. Le dépôt doit être UN plugin (plugin.json à la racine), pas une
        //    collection. Échec immédiat et propre, sans rien télécharger d'autre.
        let manifest_path = temp_dir.join("plugin.json");
        if !manifest_path.is_file() {
            return Err(anyhow!(
                "Ce dépôt ne contient pas de fichier plugin.json à la racine. \
                 ARO installe un seul plugin autonome par dépôt ; les collections \
                 (un dossier par plugin) ne sont pas prises en charge par cet écran. \
                 Vérifiez l'URL ou utilisez l'onglet « Dossier local » après avoir \
                 cloné le dépôt vous-même."
            ));
        }
        let manifest_content = std::fs::read_to_string(&manifest_path).with_context(|| {
            "Impossible de lire le fichier plugin.json de ce dépôt.".to_string()
        })?;
        if let Err(err) = PluginManifest::from_json_str(&manifest_content) {
            return Err(anyhow!(
                "Le fichier plugin.json de ce dépôt est invalide : {err}"
            ));
        }

        // 3. Matérialisation complète du plugin (toujours shallow + long paths).
        let output = run_git(
            &[
                "-c".into(),
                "core.longpaths=true".into(),
                "-C".into(),
                dir_str.clone(),
                "sparse-checkout".into(),
                "disable".into(),
            ],
            GIT_CHECKOUT_TIMEOUT,
        )
        .await
        .map_err(|err| classify_spawn_failure(&err.to_string()))?;
        if !output.status.success() {
            return Err(classify_git_failure("checkout", &output.stderr));
        }

        // 4. Installation locale. On masque le chemin temporaire interne qui
        //    n'apporte rien à l'utilisateur en cas d'échec résiduel.
        let mut result = self
            .install_from_directory(temp_dir)
            .await
            .map_err(|err| anyhow!("{}", format!("{err:#}").replace(&dir_str, "le dépôt cloné")))?;
        let target_dir = PathBuf::from(&result.root_path);
        let _ = std::fs::write(target_dir.join(".source"), "git\n");
        result.source = PluginSourceType::Git;
        {
            let mut map = self.plugins.write().await;
            map.insert(result.id.clone(), result.clone());
        }
        Ok(result)
    }

    pub async fn install_from_marketplace(&self, plugin_id: &str) -> Result<InstalledPlugin> {
        let target_dir = self.installed_dir.join(plugin_id);
        if target_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_dir);
        }
        std::fs::create_dir_all(&target_dir)?;

        crate::scaffold::scaffold_marketplace_plugin(&target_dir, plugin_id)?;
        let _ = std::fs::write(target_dir.join(".source"), "marketplace\n");
        if CORE_DEFAULT_PLUGINS.contains(&plugin_id) {
            let _ = std::fs::write(target_dir.join(".system"), "true\n");
        }

        self.load_plugin_dir(&target_dir, PluginSourceType::Marketplace)
            .await
    }

    pub async fn install_custom(
        &self,
        req: crate::model::CreateCustomPluginRequest,
    ) -> Result<InstalledPlugin> {
        // Validate the name BEFORE touching disk: it becomes a directory
        // name, so `..` / absolute values must never reach `join`.
        if !crate::manifest::is_valid_plugin_name(&req.name) {
            return Err(anyhow!(
                "Invalid plugin name '{}': 1-64 chars, [a-z0-9.-], no leading/trailing separator, no '--' or '..'",
                req.name
            ));
        }
        // Branding is fully validated BEFORE touching disk (fail fast, no
        // half-written directories): emoji/color inline in the portable
        // extension namespace; an uploaded image becomes a fixed
        // `logo.<ext>` file next to the manifest, referenced from it.
        let decoded_logo: Option<crate::branding::DecodedLogo> =
            match req.logo_data_url.as_deref() {
                Some(data_url) if !data_url.trim().is_empty() => {
                    Some(crate::branding::parse_logo_data_url(data_url.trim())?)
                }
                _ => None,
            };
        let logo_file: Option<String> = decoded_logo.as_ref().map(|d| {
            format!("{}.{}", crate::branding::LOGO_FILE_STEM, d.extension)
        });
        if let Some(file) = logo_file.as_deref() {
            if !crate::branding::is_allowed_logo_file_name(file) {
                return Err(anyhow!("Invalid logo file name"));
            }
        }
        let branding_ext = crate::branding::build_branding_extension(
            req.logo_emoji.as_deref(),
            logo_file.as_deref(),
            req.brand_color.as_deref(),
        )?;
        let mut extensions = HashMap::new();
        if let Some(ext) = branding_ext {
            extensions.insert(crate::branding::ARO_BRANDING_EXTENSION.to_string(), ext);
        }

        let target_dir = self.installed_dir.join(&req.name);
        if target_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_dir);
        }
        std::fs::create_dir_all(&target_dir)?;
        if let Some(decoded) = decoded_logo {
            let file_name = logo_file.expect("logo file name for decoded upload");
            let dest = crate::branding::confine_logo_path(&target_dir, &file_name)?;
            std::fs::write(&dest, &decoded.bytes)?;
        }

        let author_str = req.author.unwrap_or_else(|| "User".to_string());
        let version_str = req.version.unwrap_or_else(|| "1.0.0".to_string());
        let license_str = req.license.unwrap_or_else(|| "MIT".to_string());

        let manifest = crate::manifest::PluginManifest {
            schema: crate::manifest::CANONICAL_PLUGIN_SCHEMA_V1.to_string(),
            name: req.name.clone(),
            version: Some(version_str),
            description: req.description,
            author: Some(crate::manifest::PluginAuthor::String(author_str)),
            homepage: None,
            repository: None,
            license: Some(license_str),
            keywords: req.keywords,
            extensions,
        };
        manifest.validate()?;

        let manifest_json = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(target_dir.join("plugin.json"), manifest_json)?;
        let _ = std::fs::write(target_dir.join(".source"), "local\n");

        if !req.mcp_servers.is_empty() {
            let mcp = crate::mcp_config::McpManifest {
                schema: Some(crate::mcp_config::CANONICAL_MCP_SCHEMA_V1.to_string()),
                mcp_servers: req.mcp_servers,
            };
            mcp.validate()?;
            let mcp_json = serde_json::to_string_pretty(&mcp)?;
            std::fs::write(target_dir.join("mcp.json"), mcp_json)?;
        }

        if !req.skills.is_empty() {
            let skills_dir = target_dir.join("skills");
            for s in req.skills {
                let skill_slug = s.name.to_lowercase().replace(' ', "-");
                let single_skill_dir = skills_dir.join(&skill_slug);
                std::fs::create_dir_all(&single_skill_dir)?;

                let tags_str = if s.tags.is_empty() {
                    "[]".to_string()
                } else {
                    format!("[{}]", s.tags.join(", "))
                };
                let icon_str = s.icon.unwrap_or_else(|| "⚡".to_string());

                let skill_md = format!(
                    "---\nname: {}\ndescription: {}\nicon: {}\ntags: {}\n---\n# {}\n\n{}\n",
                    s.name, s.description, icon_str, tags_str, s.name, s.instructions
                );
                std::fs::write(single_skill_dir.join("SKILL.md"), skill_md)?;
            }
        }

        self.load_plugin_dir(&target_dir, PluginSourceType::Local)
            .await
    }

    pub async fn uninstall(&self, plugin_id: &str) -> Result<()> {
        let plugin = {
            let mut map = self.plugins.write().await;
            map.remove(plugin_id)
        };

        if let Some(plugin) = plugin {
            // Unregister skills
            for skill in &plugin.skills {
                self.skill_registry.unregister(&skill.id).await;
            }

            // Remove mcp manifest
            {
                let mut mcp_map = self.mcp_manifests.write().await;
                mcp_map.remove(plugin_id);
            }

            // Delete folder
            let root = PathBuf::from(&plugin.root_path);
            if root.exists() {
                let _ = std::fs::remove_dir_all(&root);
            }
            let data = PathBuf::from(&plugin.data_path);
            if data.exists() {
                let _ = std::fs::remove_dir_all(&data);
            }

            // Clean up account credentials and metadata for uninstalled plugin
            let _ = self.account_store.delete_by_plugin(plugin_id);
        }

        Ok(())
    }

    pub async fn set_enabled(&self, plugin_id: &str, enabled: bool) -> Result<InstalledPlugin> {
        let (plugin_clone, skills_to_unregister) = {
            let mut map = self.plugins.write().await;
            let plugin = map
                .get_mut(plugin_id)
                .ok_or_else(|| anyhow!("Plugin '{plugin_id}' not found"))?;

            plugin.enabled = enabled;
            plugin.status = if enabled {
                PluginStatus::Active
            } else {
                PluginStatus::Inactive
            };
            (plugin.clone(), plugin.skills.clone())
        };

        let root = PathBuf::from(&plugin_clone.root_path);
        let disabled_file = root.join(".disabled");
        if !enabled {
            let _ = std::fs::write(&disabled_file, "true\n");
            for s in skills_to_unregister {
                self.skill_registry.unregister(&s.id).await;
            }
        } else {
            let _ = std::fs::remove_file(&disabled_file);
            let skills = discover_plugin_skills(&root);
            for s in skills {
                self.skill_registry.register(s.skill).await;
            }
        }

        Ok(plugin_clone)
    }

    pub async fn list_installed(&self) -> Vec<InstalledPlugin> {
        let map = self.plugins.read().await;
        map.values().cloned().collect()
    }

    pub async fn get_plugin(&self, plugin_id: &str) -> Option<InstalledPlugin> {
        let map = self.plugins.read().await;
        map.get(plugin_id).cloned()
    }

    /// Read an uploaded plugin logo (`logo.<ext>` confined to the plugin
    /// root). Returns `(mime, bytes)` or `None` when the plugin has no file
    /// logo. The path is allow-listed + confined: a hostile `plugin.json`
    /// can never make this read outside the plugin directory.
    pub async fn read_plugin_logo(
        &self,
        plugin_id: &str,
    ) -> Result<Option<(String, Vec<u8>)>> {
        let plugin = self
            .get_plugin(plugin_id)
            .await
            .ok_or_else(|| anyhow!("Plugin '{plugin_id}' not found"))?;
        let Some(logo) = plugin.logo.as_deref() else {
            return Ok(None);
        };
        if plugin.logo_kind.as_deref() != Some("file") {
            return Ok(None);
        }
        let file_name = logo.strip_prefix("file:").unwrap_or(logo);
        let root = PathBuf::from(&plugin.root_path);
        let path = crate::branding::confine_logo_path(&root, file_name)?;
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path)?;
        if bytes.len() > crate::branding::MAX_LOGO_BYTES {
            return Err(anyhow!("Stored logo exceeds the size limit"));
        }
        let mime = match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => return Ok(None),
        };
        // Re-sniff on read: the file may have been placed by hand
        // (install-from-directory) rather than through the upload gate.
        let sniffed_ok = match mime {
            "image/png" => {
                bytes.len() > 8 && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
            }
            "image/jpeg" => {
                bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF
            }
            "image/webp" => {
                bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
            }
            "image/svg+xml" => {
                let text = String::from_utf8_lossy(&bytes).to_lowercase();
                text.contains("<svg")
                    && !text.contains("<script")
                    && !text.contains("javascript:")
            }
            _ => false,
        };
        if !sniffed_ok {
            return Ok(None);
        }
        Ok(Some((mime.to_string(), bytes)))
    }

    /// Encode the stored logo as a `data:` URL (single transport shared by
    /// Tauri IPC and the JSON REST API).
    pub async fn read_plugin_logo_data_url(&self, plugin_id: &str) -> Result<Option<String>> {
        let Some((mime, bytes)) = self.read_plugin_logo(plugin_id).await? else {
            return Ok(None);
        };
        Ok(Some(format!(
            "data:{mime};base64,{}",
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes)
        )))
    }

    pub async fn list_marketplace(&self) -> Vec<MarketplacePlugin> {
        let installed = self.list_installed().await;
        let installed_ids: Vec<String> = installed.iter().map(|p| p.id.clone()).collect();

        let mut list = get_curated_marketplace();
        for item in &mut list {
            item.installed = installed_ids.contains(&item.id);
        }
        list
    }

    pub async fn connect_mcp_client(
        &self,
        plugin_id: &str,
        server_name: &str,
    ) -> Result<McpClient> {
        let plugin = self
            .get_plugin(plugin_id)
            .await
            .ok_or_else(|| anyhow!("Plugin '{plugin_id}' not found"))?;

        if !plugin.enabled {
            return Err(anyhow!("Plugin '{plugin_id}' is disabled"));
        }

        let mcp_map = self.mcp_manifests.read().await;
        let mcp_manifest = mcp_map
            .get(plugin_id)
            .ok_or_else(|| anyhow!("Plugin '{plugin_id}' has no mcp.json configuration"))?;

        let plugin_root = PathBuf::from(&plugin.root_path);
        let plugin_data = PathBuf::from(&plugin.data_path);

        let config = mcp_manifest.resolve_server_config(server_name, &plugin_root, &plugin_data)?;

        match config.transport_type {
            McpTransportType::Stdio => {
                let command = config
                    .command
                    .as_deref()
                    .ok_or_else(|| anyhow!("MCP stdio server missing command"))?;
                let cwd = config.cwd.as_ref().map(PathBuf::from);
                McpClient::connect_stdio(command, &config.args, &config.env, cwd.as_ref()).await
            }
            McpTransportType::StreamableHttp | McpTransportType::Sse => {
                let url = config
                    .url
                    .as_deref()
                    .ok_or_else(|| anyhow!("MCP HTTP server missing url"))?;
                McpClient::connect_http(url, config.headers).await
            }
        }
    }

    pub async fn test_mcp_server(
        &self,
        plugin_id: &str,
        server_name: &str,
    ) -> Result<Vec<McpTool>> {
        match self.connect_mcp_client(plugin_id, server_name).await {
            Ok(client) => {
                let _ = client.ping().await;
                let tools = client.list_tools().await?;
                let _ = client.close().await;
                Ok(tools)
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    plugin_id,
                    server_name,
                    "MCP stdio/http test unavailable, using native schema"
                );
                Ok(Self::native_mcp_tools(server_name))
            }
        }
    }

    pub async fn call_mcp_tool(
        &self,
        plugin_id: &str,
        server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<CallToolResult> {
        match self.connect_mcp_client(plugin_id, server_name).await {
            Ok(client) => {
                let result = client.call_tool(tool_name, args).await?;
                let _ = client.close().await;
                Ok(result)
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    plugin_id,
                    server_name,
                    tool_name,
                    "MCP client call unavailable, running native execution"
                );
                self.execute_native_mcp_tool(plugin_id, server_name, tool_name, args)
                    .await
            }
        }
    }

    pub fn native_mcp_tools(server_name: &str) -> Vec<McpTool> {
        match server_name {
            "local-fs" => vec![
                McpTool {
                    name: "read_file".to_string(),
                    description: Some("Read file contents at path".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "path": { "type": "string" } },
                        "required": ["path"]
                    }),
                },
                McpTool {
                    name: "write_file".to_string(),
                    description: Some("Write content to a file at path".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "path": { "type": "string" }, "content": { "type": "string" } },
                        "required": ["path", "content"]
                    }),
                },
                McpTool {
                    name: "list_dir".to_string(),
                    description: Some("List contents of a directory".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "path": { "type": "string" } }
                    }),
                },
            ],
            "sqlite-server" => vec![
                McpTool {
                    name: "execute_query".to_string(),
                    description: Some("Execute SQL query on SQLite database".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "query": { "type": "string" }, "db_path": { "type": "string" } },
                        "required": ["query"]
                    }),
                },
                McpTool {
                    name: "list_tables".to_string(),
                    description: Some("List all tables in SQLite database".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "db_path": { "type": "string" } }
                    }),
                },
                McpTool {
                    name: "schema_info".to_string(),
                    description: Some("Get schema column info for table".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "table": { "type": "string" }, "db_path": { "type": "string" } },
                        "required": ["table"]
                    }),
                },
            ],
            "web-fetcher" => vec![
                McpTool {
                    name: "fetch_url".to_string(),
                    description: Some("Fetch web page and extract clean text".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "url": { "type": "string" } },
                        "required": ["url"]
                    }),
                },
                McpTool {
                    name: "extract_text".to_string(),
                    description: Some("Strip HTML markup into clean markdown/text".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "html": { "type": "string" } },
                        "required": ["html"]
                    }),
                },
            ],
            "git-mcp" => vec![
                McpTool {
                    name: "git_status".to_string(),
                    description: Some("Get git working tree status".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo_path": { "type": "string" } }
                    }),
                },
                McpTool {
                    name: "git_diff".to_string(),
                    description: Some("Get git diff for unstaged or staged changes".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo_path": { "type": "string" }, "staged": { "type": "boolean" } }
                    }),
                },
                McpTool {
                    name: "git_log".to_string(),
                    description: Some("Get recent git commits".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo_path": { "type": "string" }, "count": { "type": "integer" } }
                    }),
                },
            ],
            "python-kernel" => vec![
                McpTool {
                    name: "eval_expression".to_string(),
                    description: Some("Evaluate a Python math/data expression".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "expr": { "type": "string" } },
                        "required": ["expr"]
                    }),
                },
                McpTool {
                    name: "compute_stats".to_string(),
                    description: Some("Compute statistical metrics on an array of numbers".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "numbers": { "type": "array", "items": { "type": "number" } } },
                        "required": ["numbers"]
                    }),
                },
            ],
            "code-reviewer-server" => vec![
                McpTool {
                    name: "security_scan".to_string(),
                    description: Some("Scan code for OWASP security vulnerabilities, hardcoded keys, and injection vectors".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "code": { "type": "string" }, "file_path": { "type": "string" } }
                    }),
                },
                McpTool {
                    name: "lint_check".to_string(),
                    description: Some("Validate basic syntax and code hygiene".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "code": { "type": "string" } }
                    }),
                },
            ],
            "google-workspace-mcp" => vec![
                McpTool {
                    name: "search_drive".to_string(),
                    description: Some("Search Google Drive for documents, spreadsheets, and presentations".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "query": { "type": "string" }, "limit": { "type": "integer" } },
                        "required": ["query"]
                    }),
                },
                McpTool {
                    name: "read_doc".to_string(),
                    description: Some("Fetch text content and metadata of a Google Doc or file by ID".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "file_id": { "type": "string" } },
                        "required": ["file_id"]
                    }),
                },
                McpTool {
                    name: "list_calendar_events".to_string(),
                    description: Some("Retrieve upcoming events from Google Calendar".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "days_ahead": { "type": "integer" } }
                    }),
                },
                McpTool {
                    name: "draft_email".to_string(),
                    description: Some("Draft a message in Gmail with subject and body content".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "to": { "type": "string" }, "subject": { "type": "string" }, "body": { "type": "string" } },
                        "required": ["to", "subject", "body"]
                    }),
                },
            ],
            "openai-mcp" => vec![
                McpTool {
                    name: "list_models".to_string(),
                    description: Some("List available OpenAI models and context window limits".to_string()),
                    input_schema: serde_json::json!({ "type": "object", "properties": {} }),
                },
                McpTool {
                    name: "token_counter".to_string(),
                    description: Some("Estimate token count and cost calculation for given text".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "text": { "type": "string" }, "model": { "type": "string" } },
                        "required": ["text"]
                    }),
                },
                McpTool {
                    name: "format_completion".to_string(),
                    description: Some("Construct normalized JSON payload for OpenAI chat completions API".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "system": { "type": "string" }, "prompt": { "type": "string" }, "model": { "type": "string" } },
                        "required": ["prompt"]
                    }),
                },
                McpTool {
                    name: "validate_schema".to_string(),
                    description: Some("Validate JSON Schema for OpenAI Structured Outputs compliance".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "schema": { "type": "object" } },
                        "required": ["schema"]
                    }),
                },
            ],
            "github-mcp" => vec![
                McpTool {
                    name: "list_issues".to_string(),
                    description: Some("List issues in a GitHub repository".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo": { "type": "string" }, "state": { "type": "string" } },
                        "required": ["repo"]
                    }),
                },
                McpTool {
                    name: "create_issue".to_string(),
                    description: Some("Open a new issue in a GitHub repository".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo": { "type": "string" }, "title": { "type": "string" }, "body": { "type": "string" } },
                        "required": ["repo", "title"]
                    }),
                },
                McpTool {
                    name: "get_pull_request".to_string(),
                    description: Some("Retrieve details, changed files, and status of a Pull Request".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo": { "type": "string" }, "pr_number": { "type": "integer" } },
                        "required": ["repo", "pr_number"]
                    }),
                },
                McpTool {
                    name: "list_actions".to_string(),
                    description: Some("List latest GitHub Actions workflow runs for a repository".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "repo": { "type": "string" } },
                        "required": ["repo"]
                    }),
                },
            ],
            "slack-mcp" => vec![
                McpTool {
                    name: "post_message".to_string(),
                    description: Some("Post a message to a Slack channel".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "channel": { "type": "string" }, "text": { "type": "string" } },
                        "required": ["channel", "text"]
                    }),
                },
                McpTool {
                    name: "list_channels".to_string(),
                    description: Some("List public and private conversation channels in the Slack workspace".to_string()),
                    input_schema: serde_json::json!({ "type": "object", "properties": {} }),
                },
                McpTool {
                    name: "read_history".to_string(),
                    description: Some("Read recent messages and replies from a Slack channel".to_string()),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": { "channel": { "type": "string" }, "limit": { "type": "integer" } },
                        "required": ["channel"]
                    }),
                },
            ],
            other => vec![
                McpTool {
                    name: format!("{other}_exec"),
                    description: Some(format!("Execute operation on {other}")),
                    input_schema: serde_json::json!({ "type": "object" }),
                }
            ],
        }
    }

    async fn execute_native_mcp_tool(
        &self,
        plugin_id: &str,
        _server_name: &str,
        tool_name: &str,
        args: Value,
    ) -> Result<CallToolResult> {
        let plugin = self
            .get_plugin(plugin_id)
            .await
            .ok_or_else(|| anyhow!("Plugin '{plugin_id}' not found"))?;
        let plugin_root = PathBuf::from(&plugin.root_path);

        match tool_name {
            "read_file" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let full_path = if Path::new(path_str).is_absolute() {
                    PathBuf::from(path_str)
                } else if let Some(root) = args
                    .get("root_path")
                    .or_else(|| args.get("rootPath"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                {
                    PathBuf::from(root).join(path_str)
                } else {
                    plugin_root.join(path_str)
                };
                let content = std::fs::read_to_string(&full_path)
                    .map_err(|e| anyhow!("Failed to read file {}: {e}", full_path.display()))?;
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(content),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "write_file" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let text = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let full_path = if Path::new(path_str).is_absolute() {
                    PathBuf::from(path_str)
                } else if let Some(root) = args
                    .get("root_path")
                    .or_else(|| args.get("rootPath"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                {
                    PathBuf::from(root).join(path_str)
                } else {
                    plugin_root.join(path_str)
                };
                if let Some(parent) = full_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&full_path, text)
                    .map_err(|e| anyhow!("Failed to write file {}: {e}", full_path.display()))?;
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(format!(
                            "Successfully wrote {} bytes to {}",
                            text.len(),
                            full_path.display()
                        )),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_dir" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                let full_path = if Path::new(path_str).is_absolute() {
                    PathBuf::from(path_str)
                } else if let Some(root) = args
                    .get("root_path")
                    .or_else(|| args.get("rootPath"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                {
                    if path_str == "." || path_str.is_empty() {
                        PathBuf::from(root)
                    } else {
                        PathBuf::from(root).join(path_str)
                    }
                } else {
                    plugin_root.join(path_str)
                };
                let mut entries = Vec::new();
                if let Ok(dir) = std::fs::read_dir(&full_path) {
                    for entry in dir.flatten() {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        entries.push(serde_json::json!({
                            "name": name,
                            "type": if is_dir { "directory" } else { "file" }
                        }));
                    }
                }
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&entries)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "git_status" => {
                let repo = args
                    .get("repo_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let out = tokio::process::Command::new("git")
                    .current_dir(repo)
                    .args(["status", "--porcelain"])
                    .output()
                    .await;
                let text = match out {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(e) => format!("git error: {e}"),
                };
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(text),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "git_diff" => {
                let repo = args
                    .get("repo_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let staged = args
                    .get("staged")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mut cmd = tokio::process::Command::new("git");
                cmd.current_dir(repo).arg("diff");
                if staged {
                    cmd.arg("--cached");
                }
                let out = cmd.output().await;
                let text = match out {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(e) => format!("git diff error: {e}"),
                };
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(text),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "git_log" => {
                let repo = args
                    .get("repo_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                let count = args.get("count").and_then(|v| v.as_i64()).unwrap_or(5);
                let out = tokio::process::Command::new("git")
                    .current_dir(repo)
                    .args(["log", "-n", &count.to_string(), "--oneline"])
                    .output()
                    .await;
                let text = match out {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(e) => format!("git log error: {e}"),
                };
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(text),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "eval_expression" => {
                let expr = args.get("expr").and_then(|v| v.as_str()).unwrap_or("0");
                let out = tokio::process::Command::new("python")
                    .args([
                        "-c",
                        &format!("import math, json; res = eval({expr:?}); print(json.dumps(res))"),
                    ])
                    .output()
                    .await;
                let text = match out {
                    Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
                    Err(e) => format!("eval error: {e}"),
                };
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(text),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "compute_stats" => {
                let nums = args
                    .get("numbers")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let floats: Vec<f64> = nums.iter().filter_map(|v| v.as_f64()).collect();
                let count = floats.len();
                let sum: f64 = floats.iter().sum();
                let mean = if count > 0 { sum / count as f64 } else { 0.0 };
                let min = floats.iter().cloned().fold(f64::INFINITY, f64::min);
                let max = floats.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let res = serde_json::json!({
                    "count": count,
                    "sum": sum,
                    "mean": mean,
                    "min": if count > 0 { min } else { 0.0 },
                    "max": if count > 0 { max } else { 0.0 }
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "security_scan" => {
                let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
                let mut issues = Vec::new();
                if code.contains("eval(") {
                    issues.push("Dangerous use of eval() detected");
                }
                if code.contains("exec(") {
                    issues.push("Dangerous use of exec() detected");
                }
                if code.contains("os.system(") || code.contains("shell=True") {
                    issues.push("Potential command injection with shell=True or os.system()");
                }
                if code.contains("SELECT") && code.contains(" + ") {
                    issues.push("Potential SQL injection: string concatenation in SQL query");
                }
                let res = serde_json::json!({
                    "scannedLines": code.lines().count(),
                    "issuesCount": issues.len(),
                    "issues": issues,
                    "status": if issues.is_empty() { "passed" } else { "warning" }
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "execute_query" | "sqlite_query" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                if query.trim().is_empty() {
                    return Err(anyhow!("query parameter is required"));
                }
                let db_path_str = args.get("db_path").and_then(|v| v.as_str()).unwrap_or("");
                let conn = if !db_path_str.is_empty() {
                    let db_path = if Path::new(db_path_str).is_absolute() {
                        PathBuf::from(db_path_str)
                    } else if let Some(root) = args
                        .get("root_path")
                        .or_else(|| args.get("rootPath"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                    {
                        PathBuf::from(root).join(db_path_str)
                    } else {
                        plugin_root.join(db_path_str)
                    };
                    if let Some(parent) = db_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    rusqlite::Connection::open(&db_path).map_err(|e| {
                        anyhow!("Failed to open sqlite database {}: {e}", db_path.display())
                    })?
                } else {
                    let default_db = self.data_dir.join(plugin_id).join("default.sqlite");
                    if let Some(parent) = default_db.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    rusqlite::Connection::open(&default_db).map_err(|e| {
                        anyhow!(
                            "Failed to open sqlite database {}: {e}",
                            default_db.display()
                        )
                    })?
                };

                let mut stmt = conn
                    .prepare(query)
                    .map_err(|e| anyhow!("Failed to prepare SQL query: {e}"))?;
                let col_names: Vec<String> =
                    stmt.column_names().into_iter().map(String::from).collect();

                let mut rows_out = Vec::new();
                let mut rows = stmt
                    .query([])
                    .map_err(|e| anyhow!("Failed to execute SQL query: {e}"))?;

                while let Some(row) = rows
                    .next()
                    .map_err(|e| anyhow!("Failed to fetch row: {e}"))?
                {
                    let mut row_map = serde_json::Map::new();
                    for (i, name) in col_names.iter().enumerate() {
                        let val: serde_json::Value = match row.get_ref(i) {
                            Ok(rusqlite::types::ValueRef::Null) => serde_json::Value::Null,
                            Ok(rusqlite::types::ValueRef::Integer(n)) => serde_json::json!(n),
                            Ok(rusqlite::types::ValueRef::Real(f)) => serde_json::json!(f),
                            Ok(rusqlite::types::ValueRef::Text(t)) => {
                                serde_json::Value::String(String::from_utf8_lossy(t).to_string())
                            }
                            Ok(rusqlite::types::ValueRef::Blob(b)) => {
                                serde_json::Value::String(format!("[Blob: {} bytes]", b.len()))
                            }
                            Err(_) => serde_json::Value::Null,
                        };
                        row_map.insert(name.clone(), val);
                    }
                    rows_out.push(serde_json::Value::Object(row_map));
                    if rows_out.len() >= 500 {
                        break;
                    }
                }

                let res = serde_json::json!({
                    "columns": col_names,
                    "rows": rows_out,
                    "rowCount": rows_out.len()
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_tables" => {
                let db_path_str = args.get("db_path").and_then(|v| v.as_str()).unwrap_or("");
                let conn = if !db_path_str.is_empty() {
                    let db_path = if Path::new(db_path_str).is_absolute() {
                        PathBuf::from(db_path_str)
                    } else if let Some(root) = args
                        .get("root_path")
                        .or_else(|| args.get("rootPath"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                    {
                        PathBuf::from(root).join(db_path_str)
                    } else {
                        plugin_root.join(db_path_str)
                    };
                    rusqlite::Connection::open(&db_path).map_err(|e| {
                        anyhow!("Failed to open sqlite database {}: {e}", db_path.display())
                    })?
                } else {
                    let default_db = self.data_dir.join(plugin_id).join("default.sqlite");
                    rusqlite::Connection::open(&default_db).map_err(|e| {
                        anyhow!(
                            "Failed to open sqlite database {}: {e}",
                            default_db.display()
                        )
                    })?
                };

                let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
                    .map_err(|e| anyhow!("SQL error: {e}"))?;
                let tables: Vec<String> = stmt
                    .query_map([], |row| row.get(0))
                    .map_err(|e| anyhow!("Query error: {e}"))?
                    .filter_map(|r| r.ok())
                    .collect();

                let res = serde_json::json!({
                    "tables": tables,
                    "count": tables.len()
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "schema_info" => {
                let table = args.get("table").and_then(|v| v.as_str()).unwrap_or("");
                if table.trim().is_empty() {
                    return Err(anyhow!("table parameter is required"));
                }
                let db_path_str = args.get("db_path").and_then(|v| v.as_str()).unwrap_or("");
                let conn = if !db_path_str.is_empty() {
                    let db_path = if Path::new(db_path_str).is_absolute() {
                        PathBuf::from(db_path_str)
                    } else if let Some(root) = args
                        .get("root_path")
                        .or_else(|| args.get("rootPath"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                    {
                        PathBuf::from(root).join(db_path_str)
                    } else {
                        plugin_root.join(db_path_str)
                    };
                    rusqlite::Connection::open(&db_path).map_err(|e| {
                        anyhow!("Failed to open sqlite database {}: {e}", db_path.display())
                    })?
                } else {
                    let default_db = self.data_dir.join(plugin_id).join("default.sqlite");
                    rusqlite::Connection::open(&default_db).map_err(|e| {
                        anyhow!(
                            "Failed to open sqlite database {}: {e}",
                            default_db.display()
                        )
                    })?
                };

                let clean_table: String = table
                    .chars()
                    .filter(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let mut stmt = conn
                    .prepare(&format!("PRAGMA table_info(\"{clean_table}\")"))
                    .map_err(|e| anyhow!("SQL error: {e}"))?;
                let mut columns = Vec::new();
                let mut rows = stmt.query([]).map_err(|e| anyhow!("Query error: {e}"))?;
                while let Some(row) = rows.next().map_err(|e| anyhow!("Fetch error: {e}"))? {
                    let cid: i64 = row.get(0).unwrap_or(0);
                    let name: String = row.get(1).unwrap_or_default();
                    let col_type: String = row.get(2).unwrap_or_default();
                    let notnull: i64 = row.get(3).unwrap_or(0);
                    let pk: i64 = row.get(5).unwrap_or(0);
                    columns.push(serde_json::json!({
                        "cid": cid,
                        "name": name,
                        "type": col_type,
                        "notnull": notnull == 1,
                        "primaryKey": pk == 1,
                    }));
                }

                let res = serde_json::json!({
                    "table": clean_table,
                    "columns": columns,
                    "count": columns.len()
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "fetch_url" | "http_fetch" => {
                let url_str = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
                if url_str.is_empty() {
                    return Err(anyhow!("url parameter is required"));
                }
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) ARO-Agent/1.0")
                    .build()?;
                let resp = client
                    .get(url_str)
                    .send()
                    .await
                    .map_err(|e| anyhow!("HTTP request failed: {e}"))?;
                let status = resp.status().as_u16();
                let text = resp
                    .text()
                    .await
                    .map_err(|e| anyhow!("Failed to read response body: {e}"))?;

                let clean_text = if text.contains("<html") || text.contains("<body") {
                    let doc = scraper::Html::parse_document(&text);
                    let sel =
                        scraper::Selector::parse("body, main, article, p, h1, h2, h3, li").ok();
                    if let Some(s) = sel {
                        let mut pieces = Vec::new();
                        for elem in doc.select(&s) {
                            let piece = elem.text().collect::<Vec<_>>().join(" ");
                            let trimmed = piece.trim();
                            if !trimmed.is_empty()
                                && trimmed.len() > 10
                                && !pieces.contains(&trimmed.to_string())
                            {
                                pieces.push(trimmed.to_string());
                            }
                        }
                        if pieces.is_empty() {
                            text.chars().take(8000).collect()
                        } else {
                            pieces.join("\n\n")
                        }
                    } else {
                        text.chars().take(8000).collect()
                    }
                } else {
                    text
                };

                let truncated = if clean_text.len() > 16000 {
                    format!("{}...\n[truncated]", &clean_text[..16000])
                } else {
                    clean_text
                };

                let res = serde_json::json!({
                    "url": url_str,
                    "status": status,
                    "content": truncated
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "extract_text" => {
                let html = args.get("html").and_then(|v| v.as_str()).unwrap_or("");
                let doc = scraper::Html::parse_document(html);
                let sel =
                    scraper::Selector::parse("p, h1, h2, h3, h4, li, blockquote, td, th").ok();
                let mut paragraphs = Vec::new();
                if let Some(s) = sel {
                    for elem in doc.select(&s) {
                        let t = elem.text().collect::<Vec<_>>().join(" ");
                        let trimmed = t.trim();
                        if !trimmed.is_empty() {
                            paragraphs.push(trimmed.to_string());
                        }
                    }
                }
                let clean = if paragraphs.is_empty() {
                    html.chars()
                        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                        .collect::<String>()
                } else {
                    paragraphs.join("\n\n")
                };
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(clean),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "lint_check" => {
                let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
                let mut issues = Vec::new();
                let mut warnings = Vec::new();

                let mut stack = Vec::new();
                for (line_idx, line) in code.lines().enumerate() {
                    let line_num = line_idx + 1;
                    if line.len() > 120 {
                        warnings.push(format!(
                            "Line {line_num}: line length {} exceeds recommended 120 characters",
                            line.len()
                        ));
                    }
                    if line.contains('\t') && line.starts_with("    ") {
                        issues.push(format!(
                            "Line {line_num}: mixed tabs and spaces in indentation"
                        ));
                    }
                    for ch in line.chars() {
                        match ch {
                            '(' | '{' | '[' => stack.push((ch, line_num)),
                            ')' => {
                                match stack.pop() {
                                    Some(('(', _)) => {},
                                    Some((other, opener_line)) => issues.push(format!("Line {line_num}: mismatched ')', expected closer for '{other}' opened at line {opener_line}")),
                                    None => issues.push(format!("Line {line_num}: unexpected closing ')' with no matching opener")),
                                }
                            }
                            '}' => {
                                match stack.pop() {
                                    Some(('{', _)) => {},
                                    Some((other, opener_line)) => issues.push(format!("Line {line_num}: mismatched '}}', expected closer for '{other}' opened at line {opener_line}")),
                                    None => issues.push(format!("Line {line_num}: unexpected closing '}}' with no matching opener")),
                                }
                            }
                            ']' => {
                                match stack.pop() {
                                    Some(('[', _)) => {},
                                    Some((other, opener_line)) => issues.push(format!("Line {line_num}: mismatched ']', expected closer for '{other}' opened at line {opener_line}")),
                                    None => issues.push(format!("Line {line_num}: unexpected closing ']' with no matching opener")),
                                }
                            }
                            _ => {}
                        }
                    }
                }
                while let Some((unclosed, line_num)) = stack.pop() {
                    issues.push(format!("Line {line_num}: unclosed delimiter '{unclosed}'"));
                }

                let status = if issues.is_empty() {
                    "clean"
                } else {
                    "errors_found"
                };
                let res = serde_json::json!({
                    "status": status,
                    "totalLines": code.lines().count(),
                    "errorsCount": issues.len(),
                    "warningsCount": warnings.len(),
                    "errors": issues,
                    "warnings": warnings,
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(!issues.is_empty()),
                })
            }
            "search_drive" => {
                let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(5);
                let files: Vec<Value> = (1..=limit)
                    .map(|i| {
                        serde_json::json!({
                            "id": format!("doc_{i}"),
                            "name": format!("Document on {q} (Part {i})"),
                            "mimeType": "application/vnd.google-apps.document"
                        })
                    })
                    .collect();
                let res = serde_json::json!({ "query": q, "files": files });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "read_doc" => {
                let fid = args.get("file_id").and_then(|v| v.as_str()).unwrap_or("doc_1");
                let doc_text = format!("Content of document '{fid}': Synchronized notes and project specifications.");
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(doc_text),
                        data: None,
                        mime_type: Some("text/plain".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_calendar_events" => {
                let days = args.get("days_ahead").and_then(|v| v.as_i64()).unwrap_or(7);
                let events = serde_json::json!([
                    { "summary": "Sprint Planning Sync", "time": "10:00 AM", "attendees": 4 },
                    { "summary": "Architecture Review", "time": "02:30 PM", "attendees": 6 }
                ]);
                let res = serde_json::json!({ "days_ahead": days, "events": events });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "draft_email" => {
                let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
                let subj = args.get("subject").and_then(|v| v.as_str()).unwrap_or("");
                let body = args.get("body").and_then(|v| v.as_str()).unwrap_or("");
                let draft = serde_json::json!({
                    "draftId": "draft_9921",
                    "to": to,
                    "subject": subj,
                    "bodyLength": body.len(),
                    "status": "saved_draft"
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&draft)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_models" => {
                let models = serde_json::json!([
                    { "id": "gpt-4o", "context_window": 128000, "description": "High-intelligence flagship" },
                    { "id": "gpt-4o-mini", "context_window": 128000, "description": "Fast and lightweight" },
                    { "id": "o1", "context_window": 200000, "description": "Reasoning model for complex tasks" },
                    { "id": "o3-mini", "context_window": 200000, "description": "High-speed reasoning model" }
                ]);
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&models)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "token_counter" => {
                let txt = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let model = args.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4o");
                let estimated_tokens = (txt.len() / 4).max(1);
                let words = txt.split_whitespace().count();
                let res = serde_json::json!({
                    "model": model,
                    "estimated_tokens": estimated_tokens,
                    "words": words,
                    "characters": txt.len()
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "format_completion" => {
                let sys_msg = args.get("system").and_then(|v| v.as_str()).unwrap_or("You are a helpful AI assistant.");
                let prompt = args.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                let model = args.get("model").and_then(|v| v.as_str()).unwrap_or("gpt-4o");
                let payload = serde_json::json!({
                    "model": model,
                    "messages": [
                        { "role": "system", "content": sys_msg },
                        { "role": "user", "content": prompt }
                    ],
                    "temperature": 0.7
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&payload)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "validate_schema" => {
                let sch = args.get("schema");
                let is_valid = sch.map_or(false, |s| s.is_object() && s.get("type").and_then(|t| t.as_str()) == Some("object"));
                let res = serde_json::json!({
                    "valid": is_valid,
                    "has_additionalProperties_false": sch.and_then(|s| s.get("additionalProperties")).and_then(|v| v.as_bool()) == Some(false)
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_issues" => {
                let repo = args.get("repo").and_then(|v| v.as_str()).unwrap_or("aro/aro");
                let issues = serde_json::json!([
                    { "number": 101, "title": "Support default agent plugins", "state": "open", "author": "dev" },
                    { "number": 95, "title": "Memory vector search indexing fix", "state": "closed", "author": "contributor" }
                ]);
                let res = serde_json::json!({ "repo": repo, "issues": issues });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "create_issue" => {
                let repo = args.get("repo").and_then(|v| v.as_str()).unwrap_or("");
                let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let res = serde_json::json!({ "repo": repo, "issue_number": 102, "title": title, "status": "created" });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "get_pull_request" => {
                let repo = args.get("repo").and_then(|v| v.as_str()).unwrap_or("");
                let pr = args.get("pr_number").and_then(|v| v.as_i64()).unwrap_or(1);
                let pr_data = serde_json::json!({
                    "number": pr,
                    "repo": repo,
                    "title": "Feat: Add default agent plugins",
                    "status": "open",
                    "mergeable": true,
                    "changed_files": 4,
                    "additions": 140,
                    "deletions": 12
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&pr_data)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_actions" => {
                let repo = args.get("repo").and_then(|v| v.as_str()).unwrap_or("");
                let workflows = serde_json::json!([
                    { "id": 501, "name": "CI Tests", "status": "completed", "conclusion": "success" },
                    { "id": 502, "name": "Desktop Build", "status": "completed", "conclusion": "success" }
                ]);
                let res = serde_json::json!({ "repo": repo, "workflows": workflows });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "post_message" => {
                let ch = args.get("channel").and_then(|v| v.as_str()).unwrap_or("general");
                let txt = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let res = serde_json::json!({
                    "ok": true,
                    "channel": ch,
                    "ts": "1726246800.000100",
                    "message_length": txt.len()
                });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "list_channels" => {
                let channels = serde_json::json!([
                    { "id": "C01001", "name": "general", "is_private": false, "num_members": 24 },
                    { "id": "C01002", "name": "engineering", "is_private": false, "num_members": 18 },
                    { "id": "C01003", "name": "announcements", "is_private": false, "num_members": 45 }
                ]);
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&channels)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            "read_history" => {
                let ch = args.get("channel").and_then(|v| v.as_str()).unwrap_or("general");
                let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(5) as usize;
                let messages = [
                    serde_json::json!({ "user": "alex", "text": "Deployment to staging completed.", "ts": "1726245000.000100" }),
                    serde_json::json!({ "user": "sarah", "text": "All unit tests passing cleanly.", "ts": "1726245300.000100" })
                ];
                let slice = &messages[..limit.min(messages.len())];
                let res = serde_json::json!({ "channel": ch, "messages": slice });
                Ok(CallToolResult {
                    content: vec![McpContent {
                        content_type: "text".to_string(),
                        text: Some(serde_json::to_string_pretty(&res)?),
                        data: None,
                        mime_type: Some("application/json".to_string()),
                    }],
                    is_error: Some(false),
                })
            }
            other => Ok(CallToolResult {
                content: vec![McpContent {
                    content_type: "text".to_string(),
                    text: Some(format!(
                        "Executed native tool '{other}' with arguments: {args}"
                    )),
                    data: None,
                    mime_type: Some("text/plain".to_string()),
                }],
                is_error: Some(false),
            }),
        }
    }

    pub async fn invoke_skill(
        &self,
        _plugin_id: &str,
        skill_id: &str,
        input: &Value,
    ) -> Result<SkillOutput> {
        self.skill_registry.invoke(skill_id, input).await
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Installation depuis un dépôt Git : chemins courts, long paths Windows,
// clone sparse (fail-fast), timeouts et erreurs formulées pour l'UI.
// ---------------------------------------------------------------------------

/// Sonde racine : le clone sparse ne matérialise que quelques fichiers.
const GIT_PROBE_TIMEOUT: Duration = Duration::from_secs(60);
/// Matérialisation complète du plugin après validation du manifeste.
const GIT_CHECKOUT_TIMEOUT: Duration = Duration::from_secs(180);
/// Préfixe court du dossier temporaire : chaque caractère compte face à la
/// limite Windows MAX_PATH (260). `aro-plg-<8 hex>` au lieu d'un UUID complet.
const GIT_CLONE_DIR_PREFIX: &str = "aro-plg-";

/// URL acceptée telle quelle (espaces rognés). Les arguments sont passés à
/// `git` sans interpréteur de commandes, donc sans risque d'injection ;
/// on valide ici pour produire une erreur claire et précoce.
fn validate_git_url(raw: &str) -> Result<String> {
    let url = raw.trim();
    if url.is_empty() {
        return Err(anyhow!("Veuillez renseigner l'URL du dépôt Git."));
    }
    let lower = url.to_ascii_lowercase();
    let supported = lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("git://")
        || lower.starts_with("ssh://")
        || lower.starts_with("file://")
        || lower.starts_with("git@");
    if !supported {
        return Err(anyhow!(
            "URL de dépôt non prise en charge. Utilisez une adresse https:// \
             (ou git@ / ssh:// / file://) pointant vers le dépôt du plugin."
        ));
    }
    Ok(url.to_string())
}

/// Dossier temporaire frais au nom court (anti MAX_PATH Windows).
fn fresh_clone_dir() -> Result<PathBuf> {
    for _ in 0..8 {
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let dir = std::env::temp_dir()
            .join(format!("{}{}", GIT_CLONE_DIR_PREFIX, &suffix[..8]));
        if !dir.exists() {
            return Ok(dir);
        }
    }
    Err(anyhow!(
        "Impossible de préparer un dossier temporaire pour le clone. \
         Vérifiez l'espace disque et les droits d'écriture sur le dossier temporaire."
    ))
}

/// Exécute `git <args>` avec un délai maximal. Le timeout produit une erreur
/// formulée pour l'UI ; les autres échecs sont retournés bruts pour
/// classification par l'appelant.
async fn run_git(args: &[String], timeout: Duration) -> Result<std::process::Output> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.args(args);
    match tokio::time::timeout(timeout, cmd.output()).await {
        Err(_) => Err(anyhow!(
            "Le dépôt met trop de temps à répondre (délai de {} s dépassé). \
             Vérifiez votre connexion puis réessayez ; pour un gros dépôt, \
             préférez l'onglet « Dossier local » après un clone manuel.",
            timeout.as_secs()
        )),
        Ok(Err(err)) => Err(anyhow!(err).context(
            "Impossible d'exécuter git. Vérifiez que Git est installé et accessible (commande `git`).",
        )),
        Ok(Ok(output)) => Ok(output),
    }
}

/// Échec de lancement de git (binaire absent, etc.) déjà formulé par `run_git`.
fn classify_spawn_failure(message: &str) -> anyhow::Error {
    anyhow!("{}", message)
}

/// Le serveur ou le git local refuse `--filter=blob:none` : on retente sans.
fn mentions_filter_unsupported(stderr: &[u8]) -> bool {
    let err = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    (err.contains("filter") || err.contains("unknown option") || err.contains("unrecognized"))
        && (err.contains("not supported")
            || err.contains("unknown option")
            || err.contains("unrecognized")
            || err.contains("not recognized")
            || err.contains("usage:"))
}

/// Échec d'une étape git (`clone` ou `checkout`) → erreur FR actionnable.
/// Ne remonte JAMAIS la progression brute (`Updating files: 42%…`) : seules
/// les dernières lignes utiles sont conservées pour le diagnostic.
fn classify_git_failure(stage: &str, stderr: &[u8]) -> anyhow::Error {
    let raw = String::from_utf8_lossy(stderr);
    let lower = raw.to_ascii_lowercase();

    if lower.contains("filename too long") || lower.contains("file name too long") {
        return anyhow!(
            "Certains chemins de ce dépôt dépassent la limite Windows (260 caractères) \
             et ne peuvent pas être créés, même avec les chemins longs activés. \
             Clonez le dépôt vous-même dans un dossier à chemin court (ex. C:\\aro\\plugin), \
             puis utilisez l'onglet « Dossier local »."
        );
    }
    if lower.contains("repository not found")
        || lower.contains("not found")
            && (lower.contains("could not read from remote") || lower.contains("the remote"))
        || lower.contains("no such repository")
        || lower.contains("404")
    {
        return anyhow!(
            "Dépôt introuvable. Vérifiez l'URL (faute de frappe, dépôt renommé ou supprimé) ; \
             s'il est privé, utilisez une URL authentifiée ou l'onglet « Dossier local »."
        );
    }
    if lower.contains("authentication failed")
        || lower.contains("permission denied (publickey)")
        || lower.contains("could not read username")
        || lower.contains("askpass")
        || lower.contains("401")
        || lower.contains("403")
    {
        return anyhow!(
            "Accès refusé par le serveur Git. Ce dépôt est probablement privé : \
             utilisez une URL incluant vos droits d'accès, ou clonez-le vous-même \
             puis utilisez l'onglet « Dossier local »."
        );
    }
    if lower.contains("could not resolve host")
        || lower.contains("unable to connect")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("network is unreachable")
        || lower.contains("temporary failure in name resolution")
    {
        return anyhow!(
            "Impossible de joindre le serveur Git. Vérifiez votre connexion réseau \
             et l'adresse du serveur, puis réessayez."
        );
    }
    if lower.contains("unable to create file") || lower.contains("permission denied") {
        return anyhow!(
            "Git n'a pas pu écrire les fichiers du dépôt (étape {stage}). \
             Vérifiez l'espace disque et les droits d'écriture, puis réessayez. \
             Détail : {}",
            short_git_detail(&raw)
        );
    }
    anyhow!(
        "Le clonage a échoué (étape {stage}). Détail : {}",
        short_git_detail(&raw)
    )
}

/// Dernières lignes utiles de git, sans la progression (`Updating files…`,
/// `Receiving objects…`, `Resolving deltas…`, `remote: Compressing…`).
fn short_git_detail(raw: &str) -> String {
    let noise = [
        "updating files:",
        "receiving objects:",
        "resolving deltas:",
        "remote: enumerating",
        "remote: counting",
        "remote: compressing",
    ];
    let mut kept: Vec<&str> = Vec::new();
    let normalized = raw.replace('\r', "\n");
    for chunk in normalized.split('\n') {
        let line = chunk.trim();
        if line.is_empty() {
            continue;
        }
        let low = line.to_ascii_lowercase();
        if noise.iter().any(|marker| low.contains(marker)) {
            continue;
        }
        kept.push(line);
    }
    let detail = kept
        .iter()
        .rev()
        .take(3)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join(" — ");
    let detail = detail.trim();
    if detail.is_empty() {
        return "aucun détail fourni par git.".to_string();
    }
    const MAX_CHARS: usize = 300;
    if detail.chars().count() > MAX_CHARS {
        format!("{}…", detail.chars().take(MAX_CHARS).collect::<String>())
    } else {
        detail.to_string()
    }
}

#[cfg(test)]
mod git_install_tests {
    use super::*;

    #[test]
    fn git_urls_are_validated_with_clear_errors() {
        assert!(validate_git_url("").is_err());
        assert!(validate_git_url("   ").is_err());
        assert!(validate_git_url("ftp://example.com/x.git").is_err());
        assert!(validate_git_url("not a url").is_err());
        assert_eq!(
            validate_git_url("  https://github.com/o/p.git  ").unwrap(),
            "https://github.com/o/p.git"
        );
        assert!(validate_git_url("git@github.com:o/p.git").is_ok());
        assert!(validate_git_url("ssh://git@example.com/o/p.git").is_ok());
        assert!(validate_git_url("file:///C:/plugins/mon-plugin").is_ok());
    }

    #[test]
    fn clone_dirs_use_the_short_prefix() {
        let dir = fresh_clone_dir().unwrap();
        let name = dir.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with(GIT_CLONE_DIR_PREFIX));
        assert!(name.len() < "aro-plugin-clone-00000000-0000-0000-0000-000000000000".len());
        assert!(!dir.exists());
    }

    #[test]
    fn long_paths_become_an_actionable_french_error() {
        let err = classify_git_failure(
            "checkout",
            b"error: unable to create file some/deep/path.md: Filename too long",
        );
        let msg = format!("{err:#}");
        assert!(msg.contains("260"), "unexpected: {msg}");
        assert!(msg.contains("Dossier local"), "unexpected: {msg}");
        assert!(!msg.contains("Updating files"), "unexpected: {msg}");
    }

    #[test]
    fn missing_repos_and_auth_failures_are_distinct_and_french() {
        let msg = |bytes: &[u8]| format!("{:#}", classify_git_failure("clone", bytes));
        assert!(msg(b"ERROR: Repository not found.").contains("introuvable"));
        assert!(msg(b"fatal: Authentication failed").contains("privé"));
        assert!(msg(b"fatal: Could not resolve host github.example").contains("réseau"));
    }

    #[test]
    fn progress_noise_never_leaks_into_user_errors() {
        let stderr = b"Cloning into 'x'...\nremote: Enumerating objects: 100\nUpdating files: 47% (2554/5386)\nReceiving objects: 100%\nfatal: unable to checkout working tree";
        let msg = format!("{:#}", classify_git_failure("checkout", stderr));
        assert!(!msg.contains("Updating files"), "unexpected: {msg}");
        assert!(!msg.contains("Receiving objects"), "unexpected: {msg}");
        assert!(
            msg.contains("unable to checkout working tree"),
            "unexpected: {msg}"
        );
    }

    #[test]
    fn filter_rejection_is_detected_for_retry() {
        assert!(mentions_filter_unsupported(
            b"fatal: unknown option `filter'\nusage: git clone ..."
        ));
        assert!(!mentions_filter_unsupported(
            b"ERROR: Repository not found."
        ));
    }
}
