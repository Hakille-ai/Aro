import os

file_path = r'c:\Users\Stagiaire\Documents\ARO\crates\aro-store\src\lib.rs'

with open(file_path, 'r', encoding='utf-8') as f:
    content = f.read()

# 1. Update the PersistedCollection enum definition
old_enum = """pub enum PersistedCollection {
    Memories,
    Personalities,
    SystemPrompts,
    VoiceProfiles,
    SkillGroups,
    Skills,
    PluginConnections,
    McpServers,
    Hooks,
    ScheduledTasks,
    Teams,
    UsageEvents,
}"""

new_enum = """pub enum PersistedCollection {
    Memories,
    Personalities,
    SystemPrompts,
    VoiceProfiles,
    SkillGroups,
    Skills,
    PluginConnections,
    McpServers,
    Hooks,
    ScheduledTasks,
    Teams,
    UsageEvents,
    CustomAgentDefinitions,
    CustomModelDefinitions,
}"""

if old_enum in content:
    content = content.replace(old_enum, new_enum)
else:
    print("Warning: old_enum not found")

# 2. Update PersistedCollection::from_slug
old_from_slug = """            "teams" => Some(Self::Teams),
            "usage" | "usage-events" => Some(Self::UsageEvents),
            _ => None,"""

new_from_slug = """            "teams" => Some(Self::Teams),
            "usage" | "usage-events" => Some(Self::UsageEvents),
            "agent-definitions" | "agents" => Some(Self::CustomAgentDefinitions),
            "model-definitions" | "models" => Some(Self::CustomModelDefinitions),
            _ => None,"""

if old_from_slug in content:
    content = content.replace(old_from_slug, new_from_slug)
else:
    print("Warning: old_from_slug not found")

# 3. Update PersistedCollection::table_name
old_table_name = """            Self::Teams => "teams",
            Self::UsageEvents => "usage_events","""

new_table_name = """            Self::Teams => "teams",
            Self::UsageEvents => "usage_events",
            Self::CustomAgentDefinitions => "custom_agent_definitions",
            Self::CustomModelDefinitions => "custom_model_definitions","""

if old_table_name in content:
    content = content.replace(old_table_name, new_table_name)
else:
    print("Warning: old_table_name not found")

# 4. Update PersistedCollection::slug
old_slug = """            Self::Teams => "teams",
            Self::UsageEvents => "usage","""

new_slug = """            Self::Teams => "teams",
            Self::UsageEvents => "usage",
            Self::CustomAgentDefinitions => "agent-definitions",
            Self::CustomModelDefinitions => "model-definitions","""

if old_slug in content:
    content = content.replace(old_slug, new_slug)
else:
    print("Warning: old_slug not found")

# 5. Update PersistedCollection::is_user_owned
old_is_user_owned = """    fn is_user_owned(self) -> bool {
        matches!(
            self,
            Self::Memories | Self::Personalities | Self::SystemPrompts | Self::VoiceProfiles
        )
    }"""

new_is_user_owned = """    fn is_user_owned(self) -> bool {
        matches!(
            self,
            Self::Memories | Self::Personalities | Self::SystemPrompts | Self::VoiceProfiles | Self::CustomAgentDefinitions | Self::CustomModelDefinitions
        )
    }"""

if old_is_user_owned in content:
    content = content.replace(old_is_user_owned, new_is_user_owned)
else:
    print("Warning: old_is_user_owned not found")

# 6. Update create_collection_item match
old_create_match = """            PersistedCollection::Teams => self.create_team(user_id, organization_id, payload).await,
            PersistedCollection::UsageEvents => {
                self.create_usage_event(organization_id, user_id, payload)
                    .await
            }"""

new_create_match = """            PersistedCollection::Teams => self.create_team(user_id, organization_id, payload).await,
            PersistedCollection::UsageEvents => {
                self.create_usage_event(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::CustomAgentDefinitions => {
                self.create_custom_agent_definition(organization_id, user_id, payload)
                    .await
            }
            PersistedCollection::CustomModelDefinitions => {
                self.create_custom_model_definition(organization_id, user_id, payload)
                    .await
            }"""

if old_create_match in content:
    content = content.replace(old_create_match, new_create_match)
else:
    print("Warning: old_create_match not found")

# 7. Update update_collection_item match
old_update_match = """            PersistedCollection::SkillGroups => {
                self.update_skill_group(organization_id, item_id, payload)
                    .await
            }"""

new_update_match = """            PersistedCollection::SkillGroups => {
                self.update_skill_group(organization_id, item_id, payload)
                    .await
            }
            PersistedCollection::CustomAgentDefinitions => {
                self.update_custom_agent_definition(organization_id, user_id, item_id, payload)
                    .await
            }
            PersistedCollection::CustomModelDefinitions => {
                self.update_custom_model_definition(organization_id, user_id, item_id, payload)
                    .await
            }"""

if old_update_match in content:
    content = content.replace(old_update_match, new_update_match)
else:
    print("Warning: old_update_match not found")

# 8. Add helper optional_string_vec
old_helper = """fn optional_uuid_if_valid(payload: &Value, keys: &[&str]) -> Option<Uuid> {
    optional_string(payload, keys).and_then(|value| Uuid::parse_str(&value).ok())
}"""

new_helper = """fn optional_uuid_if_valid(payload: &Value, keys: &[&str]) -> Option<Uuid> {
    optional_string(payload, keys).and_then(|value| Uuid::parse_str(&value).ok())
}

fn optional_string_vec(payload: &Value, keys: &[&str]) -> Option<Vec<String>> {
    value_for(payload, keys).and_then(|value| match value {
        Value::Array(items) => Some(
            items
                .iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<Vec<_>>(),
        ),
        Value::String(val) => Some(vec![val.clone()]),
        _ => None,
    })
}"""

if old_helper in content:
    content = content.replace(old_helper, new_helper)
else:
    print("Warning: old_helper not found")

# 9. Add the CRUD methods to impl AroStore (we can append them before the helper functions)
crud_methods = """
    async fn create_custom_agent_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let name = required_string(&payload, &["name"])?;
        let description = optional_string(&payload, &["description"]);
        let system_prompt = optional_string(&payload, &["systemPrompt", "system_prompt"]);
        let model_provider_id = optional_string(&payload, &["modelProviderId", "model_provider_id"]);
        let model_id = optional_string(&payload, &["modelId", "model_id"]);
        let autonomy_profile_id = optional_uuid_if_valid(&payload, &["autonomyProfileId", "autonomy_profile_id"]);
        let enabled_tools = optional_string_vec(&payload, &["enabledTools", "enabled_tools"]).unwrap_or_default();

        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO custom_agent_definitions
                (organization_id, owner_user_id, name, description, system_prompt, model_provider_id, model_id, autonomy_profile_id, enabled_tools)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(name)
        .bind(description)
        .bind(system_prompt)
        .bind(model_provider_id)
        .bind(model_id)
        .bind(autonomy_profile_id)
        .bind(enabled_tools)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_custom_agent_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let name = optional_string(&payload, &["name"]);
        let description = optional_string(&payload, &["description"]);
        let system_prompt = optional_string(&payload, &["systemPrompt", "system_prompt"]);
        let model_provider_id = optional_string(&payload, &["modelProviderId", "model_provider_id"]);
        let model_id = optional_string(&payload, &["modelId", "model_id"]);
        let autonomy_profile_id = autonomy_profile_id_param(&payload);
        let enabled_tools = optional_string_vec(&payload, &["enabledTools", "enabled_tools"]);

        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE custom_agent_definitions
              SET
                name = COALESCE($4, name),
                description = COALESCE($5, description),
                system_prompt = COALESCE($6, system_prompt),
                model_provider_id = COALESCE($7, model_provider_id),
                model_id = COALESCE($8, model_id),
                autonomy_profile_id = COALESCE($9, autonomy_profile_id),
                enabled_tools = COALESCE($10, enabled_tools),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(name)
        .bind(description)
        .bind(system_prompt)
        .bind(model_provider_id)
        .bind(model_id)
        .bind(autonomy_profile_id)
        .bind(enabled_tools)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn create_custom_model_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let provider_kind = required_string(&payload, &["providerKind", "provider_kind"])?;
        let provider_id = required_string(&payload, &["providerId", "provider_id"])?;
        let model_id = required_string(&payload, &["modelId", "model_id"])?;
        let label = required_string(&payload, &["label"])?;
        let endpoint = required_string(&payload, &["endpoint"])?;
        let api_key_vault_key = optional_string(&payload, &["apiKeyVaultKey", "api_key_vault_key"]);

        let row = sqlx::query(
            r#"
            WITH inserted AS (
              INSERT INTO custom_model_definitions
                (organization_id, owner_user_id, provider_kind, provider_id, model_id, label, endpoint, api_key_vault_key)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
              RETURNING *
            )
            SELECT to_jsonb(inserted) AS data FROM inserted
            "#,
        )
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(provider_kind)
        .bind(provider_id)
        .bind(model_id)
        .bind(label)
        .bind(endpoint)
        .bind(api_key_vault_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }

    async fn update_custom_model_definition(
        &self,
        organization_id: Uuid,
        owner_user_id: Uuid,
        item_id: Uuid,
        payload: Value,
    ) -> AroResult<Value> {
        let provider_kind = optional_string(&payload, &["providerKind", "provider_kind"]);
        let provider_id = optional_string(&payload, &["providerId", "provider_id"]);
        let model_id = optional_string(&payload, &["modelId", "model_id"]);
        let label = optional_string(&payload, &["label"]);
        let endpoint = optional_string(&payload, &["endpoint"]);
        let api_key_vault_key = optional_string(&payload, &["apiKeyVaultKey", "api_key_vault_key"]);

        let row = sqlx::query(
            r#"
            WITH updated AS (
              UPDATE custom_model_definitions
              SET
                provider_kind = COALESCE($4, provider_kind),
                provider_id = COALESCE($5, provider_id),
                model_id = COALESCE($6, model_id),
                label = COALESCE($7, label),
                endpoint = COALESCE($8, endpoint),
                api_key_vault_key = COALESCE($9, api_key_vault_key),
                updated_at = now()
              WHERE id = $1 AND organization_id = $2 AND owner_user_id = $3 AND deleted_at IS NULL
              RETURNING *
            )
            SELECT to_jsonb(updated) AS data FROM updated
            "#,
        )
        .bind(item_id)
        .bind(organization_id)
        .bind(owner_user_id)
        .bind(provider_kind)
        .bind(provider_id)
        .bind(model_id)
        .bind(label)
        .bind(endpoint)
        .bind(api_key_vault_key)
        .fetch_one(&self.pool)
        .await
        .map_err(map_sqlx)?;
        Ok(row.get("data"))
    }
"""

# Let's write autonomy_profile_id parser for update since it can be either Value::Null or a UUID.
# Wait, let's define it inside our script to be appended before helper functions:
helper_funcs = """fn autonomy_profile_id_param(payload: &Value) -> Option<Option<Uuid>> {
    value_for(payload, &["autonomyProfileId", "autonomy_profile_id"]).map(|val| {
        match val {
            Value::Null => None,
            Value::String(s) => Uuid::parse_str(s).ok(),
            _ => None,
        }
    })
}"""

content = content.replace("fn required_string", crud_methods + "\\n" + helper_funcs + "\\nfn required_string", 1)

with open(file_path, 'w', encoding='utf-8') as f:
    f.write(content)

print("lib.rs updated successfully.")
