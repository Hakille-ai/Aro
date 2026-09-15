use std::{collections::HashMap, env, sync::Arc, time::Duration};

use aro_core::{
    compute_memory_utility, compute_recency_decay, AroError, AroResult, ContextSource,
    LongTermMemory, MEMORY_STATUS_APPROVED,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VectorMemoryMode {
    Disabled,
    Auto,
    Required,
}

impl VectorMemoryMode {
    fn from_env_value(value: Option<String>) -> Self {
        match value
            .unwrap_or_else(|| "auto".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "disabled" | "off" | "false" | "0" => Self::Disabled,
            "required" | "strict" => Self::Required,
            _ => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddingProviderKind {
    Ollama,
    Mock,
}

impl EmbeddingProviderKind {
    fn from_env_value(value: Option<String>) -> Self {
        match value
            .unwrap_or_else(|| "ollama".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "mock" => Self::Mock,
            _ => Self::Ollama,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Mock => "mock",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorMemoryConfig {
    pub mode: VectorMemoryMode,
    pub qdrant_url: String,
    pub qdrant_api_key: Option<String>,
    pub embedding_provider: EmbeddingProviderKind,
    pub embedding_model: String,
    pub ollama_endpoint: String,
    pub request_timeout_ms: u64,
}

impl VectorMemoryConfig {
    pub fn from_env() -> Self {
        Self {
            mode: VectorMemoryMode::from_env_value(env::var("ARO_VECTOR_MEMORY_MODE").ok()),
            qdrant_url: env::var("ARO_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string()),
            qdrant_api_key: env::var("ARO_QDRANT_API_KEY")
                .ok()
                .and_then(non_empty_string),
            embedding_provider: EmbeddingProviderKind::from_env_value(
                env::var("ARO_EMBEDDING_PROVIDER").ok(),
            ),
            embedding_model: env::var("ARO_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            ollama_endpoint: env::var("ARO_OLLAMA_ENDPOINT")
                .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string()),
            request_timeout_ms: env::var("ARO_VECTOR_TIMEOUT_MS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(5_000),
        }
    }
}

fn non_empty_string(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryVectorScope {
    pub namespace: String,
    pub organization_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
}

impl MemoryVectorScope {
    pub fn local() -> Self {
        Self {
            namespace: "local".to_string(),
            organization_id: None,
            owner_user_id: None,
        }
    }

    pub fn cloud(organization_id: Uuid, owner_user_id: Uuid) -> Self {
        Self {
            namespace: "cloud".to_string(),
            organization_id: Some(organization_id),
            owner_user_id: Some(owner_user_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryIndexStatus {
    pub mode: VectorMemoryMode,
    pub state: String,
    pub qdrant_url: String,
    pub collection_name: Option<String>,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub dimension: Option<usize>,
    pub indexed_count: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReindexReport {
    pub status: MemoryIndexStatus,
    pub indexed_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone)]
pub struct VectorMemoryHit {
    pub memory_id: Uuid,
    pub score: f32,
}

pub const DEFAULT_RRF_K: f32 = 60.0;
pub const DEFAULT_WEIGHT_FTS: f32 = 0.40;
pub const DEFAULT_WEIGHT_VEC: f32 = 0.60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FusedMemoryScore {
    pub memory_id: Uuid,
    pub rrf_score: f32,
    pub fts_rank: Option<usize>,
    pub vec_rank: Option<usize>,
}

/// Computes Reciprocal Rank Fusion (RRF) over full-text and vector search hit lists.
///
/// Formula:
///   RRF_Score(d) = sum_{m in {fts, vec}} (w_m / (k + rank_m(d)))
///
/// - Rank is 1-based.
/// - Omit term for missing items (rank treated as infinity).
/// - Clamped: k >= 1.0, w_fts >= 0.0, w_vec >= 0.0.
/// - Duplicate IDs in the same list retain the best (minimum) rank.
/// - Returns list sorted descending by rrf_score with deterministic tie-breaking.
pub fn reciprocal_rank_fusion(
    fts_hits: &[Uuid],
    vec_hits: &[VectorMemoryHit],
    k: f32,
    w_fts: f32,
    w_vec: f32,
) -> Vec<FusedMemoryScore> {
    let k = k.max(1.0);
    let w_fts = w_fts.max(0.0);
    let w_vec = w_vec.max(0.0);

    let mut fts_map: HashMap<Uuid, usize> = HashMap::new();
    for (i, &id) in fts_hits.iter().enumerate() {
        fts_map.entry(id).or_insert(i + 1);
    }

    let mut vec_map: HashMap<Uuid, usize> = HashMap::new();
    for (i, hit) in vec_hits.iter().enumerate() {
        vec_map.entry(hit.memory_id).or_insert(i + 1);
    }

    let mut all_ids: Vec<Uuid> = fts_map.keys().chain(vec_map.keys()).copied().collect();
    all_ids.sort();
    all_ids.dedup();

    let mut fused: Vec<FusedMemoryScore> = Vec::with_capacity(all_ids.len());

    for id in all_ids {
        let mut rrf_score = 0.0_f32;
        let fts_rank = fts_map.get(&id).copied();
        let vec_rank = vec_map.get(&id).copied();

        if let Some(rank) = fts_rank {
            rrf_score += w_fts / (k + rank as f32);
        }

        if let Some(rank) = vec_rank {
            rrf_score += w_vec / (k + rank as f32);
        }

        fused.push(FusedMemoryScore {
            memory_id: id,
            rrf_score,
            fts_rank,
            vec_rank,
        });
    }

    fused.sort_by(|a, b| {
        b.rrf_score
            .partial_cmp(&a.rrf_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                a.vec_rank
                    .unwrap_or(usize::MAX)
                    .cmp(&b.vec_rank.unwrap_or(usize::MAX))
            })
            .then_with(|| {
                a.fts_rank
                    .unwrap_or(usize::MAX)
                    .cmp(&b.fts_rank.unwrap_or(usize::MAX))
            })
            .then_with(|| a.memory_id.cmp(&b.memory_id))
    });

    fused
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, input: &str) -> AroResult<Vec<f32>>;
    fn provider_id(&self) -> &'static str;
    fn model_id(&self) -> &str;
}

#[derive(Clone)]
pub struct OllamaEmbeddingProvider {
    client: Client,
    endpoint: String,
    model: String,
}

impl OllamaEmbeddingProvider {
    pub fn new(endpoint: impl Into<String>, model: impl Into<String>, timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            endpoint: endpoint.into().trim_end_matches('/').to_string(),
            model: model.into(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for OllamaEmbeddingProvider {
    async fn embed(&self, input: &str) -> AroResult<Vec<f32>> {
        let response = self
            .client
            .post(format!("{}/api/embed", self.endpoint))
            .json(&json!({
                "model": self.model,
                "input": input,
            }))
            .send()
            .await;

        if let Ok(response) = response {
            if response.status().is_success() {
                let body = response
                    .json::<OllamaEmbedResponse>()
                    .await
                    .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
                if let Some(vector) = body.embeddings.into_iter().next() {
                    return normalize_embedding(vector);
                }
            }
        }

        let response = self
            .client
            .post(format!("{}/api/embeddings", self.endpoint))
            .json(&json!({
                "model": self.model,
                "prompt": input,
            }))
            .send()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;

        if !response.status().is_success() {
            return Err(AroError::RuntimeUnavailable(format!(
                "Ollama embeddings returned {}",
                response.status()
            )));
        }

        let body = response
            .json::<OllamaLegacyEmbeddingResponse>()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
        normalize_embedding(body.embedding)
    }

    fn provider_id(&self) -> &'static str {
        "ollama"
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct OllamaLegacyEmbeddingResponse {
    embedding: Vec<f32>,
}

#[derive(Clone)]
pub struct MockEmbeddingProvider {
    model: String,
    dimensions: usize,
}

impl MockEmbeddingProvider {
    pub fn new(model: impl Into<String>, dimensions: usize) -> Self {
        Self {
            model: model.into(),
            dimensions: dimensions.max(8),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, input: &str) -> AroResult<Vec<f32>> {
        let mut vector = vec![0.0_f32; self.dimensions];
        for token in input.split_whitespace() {
            let hash = Sha256::digest(token.to_ascii_lowercase().as_bytes());
            let index = u16::from_be_bytes([hash[0], hash[1]]) as usize % self.dimensions;
            let sign = if hash[2] % 2 == 0 { 1.0 } else { -1.0 };
            vector[index] += sign;
        }
        normalize_embedding(vector)
    }

    fn provider_id(&self) -> &'static str {
        "mock"
    }

    fn model_id(&self) -> &str {
        &self.model
    }
}

fn normalize_embedding(mut vector: Vec<f32>) -> AroResult<Vec<f32>> {
    if vector.is_empty() {
        return Err(AroError::RuntimeUnavailable(
            "embedding provider returned an empty vector".to_string(),
        ));
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    Ok(vector)
}

#[derive(Clone)]
pub struct QdrantMemoryIndex {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

impl QdrantMemoryIndex {
    pub fn new(config: &VectorMemoryConfig) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_millis(config.request_timeout_ms))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            client,
            base_url: normalize_qdrant_http_url(&config.qdrant_url),
            api_key: config.qdrant_api_key.clone(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn ensure_collection(
        &self,
        collection_name: &str,
        dimension: usize,
    ) -> AroResult<()> {
        let get = self
            .request(
                reqwest::Method::GET,
                &format!("/collections/{collection_name}"),
            )
            .send()
            .await
            .map_err(map_reqwest)?;
        if get.status().is_success() {
            return Ok(());
        }
        if get.status() != StatusCode::NOT_FOUND {
            return Err(qdrant_status_error("checking collection", get.status()).await);
        }

        let create = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{collection_name}"),
            )
            .json(&json!({
                "vectors": {
                    "size": dimension,
                    "distance": "Cosine"
                },
                "on_disk_payload": true
            }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if !create.status().is_success() {
            return Err(qdrant_status_error("creating collection", create.status()).await);
        }

        for (field, schema) in [
            ("namespace", "keyword"),
            ("organizationId", "keyword"),
            ("ownerUserId", "keyword"),
            ("memoryId", "keyword"),
            ("status", "keyword"),
            ("scope", "keyword"),
            ("category", "keyword"),
            ("embeddingModel", "keyword"),
            ("pinned", "bool"),
            ("salience", "float"),
        ] {
            let _ = self
                .request(
                    reqwest::Method::PUT,
                    &format!("/collections/{collection_name}/index"),
                )
                .json(&json!({
                    "field_name": field,
                    "field_schema": schema
                }))
                .send()
                .await;
        }
        Ok(())
    }

    pub async fn upsert_memory(
        &self,
        collection_name: &str,
        memory: &LongTermMemory,
        vector: Vec<f32>,
        scope: &MemoryVectorScope,
        embedding_model: &str,
    ) -> AroResult<()> {
        let payload = memory_payload(memory, scope, embedding_model);
        let response = self
            .request(
                reqwest::Method::PUT,
                &format!("/collections/{collection_name}/points"),
            )
            .json(&json!({
                "points": [{
                    "id": memory.id.to_string(),
                    "vector": vector,
                    "payload": payload
                }]
            }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(qdrant_status_error("upserting memory point", response.status()).await)
        }
    }

    pub async fn delete_memory(
        &self,
        collection_name: &str,
        memory_id: Uuid,
        _scope: &MemoryVectorScope,
    ) -> AroResult<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{collection_name}/points/delete?wait=true"),
            )
            .json(&json!({
                "points": [memory_id.to_string()]
            }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(qdrant_status_error("deleting memory point", response.status()).await)
        }
    }

    pub async fn clear_scope(
        &self,
        collection_name: &str,
        scope: &MemoryVectorScope,
    ) -> AroResult<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{collection_name}/points/delete?wait=true"),
            )
            .json(&json!({
                "filter": scope_filter(scope)
            }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(qdrant_status_error("clearing memory points", response.status()).await)
        }
    }

    pub async fn search(
        &self,
        collection_name: &str,
        query_vector: Vec<f32>,
        scope: &MemoryVectorScope,
        limit: usize,
    ) -> AroResult<Vec<VectorMemoryHit>> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/collections/{collection_name}/points/search"),
            )
            .json(&json!({
                "vector": query_vector,
                "filter": search_filter(scope),
                "limit": limit.max(1),
                "with_payload": true,
                "with_vector": false
            }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(qdrant_status_error("searching memory points", response.status()).await);
        }
        let body = response
            .json::<QdrantSearchResponse>()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
        Ok(body
            .result
            .into_iter()
            .filter_map(|point| {
                let memory_id = point
                    .payload
                    .get("memoryId")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .or_else(|| point_id_to_uuid(&point.id));
                memory_id.map(|memory_id| VectorMemoryHit {
                    memory_id,
                    score: point.score,
                })
            })
            .collect())
    }

    pub async fn collection_count(&self, collection_name: &str) -> AroResult<Option<u64>> {
        let response = self
            .request(
                reqwest::Method::GET,
                &format!("/collections/{collection_name}"),
            )
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let value = response
            .json::<Value>()
            .await
            .map_err(|err| AroError::RuntimeUnavailable(err.to_string()))?;
        Ok(value
            .pointer("/result/points_count")
            .and_then(Value::as_u64)
            .or_else(|| {
                value
                    .pointer("/result/vectors_count")
                    .and_then(Value::as_u64)
            }))
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let request = self
            .client
            .request(method, format!("{}{}", self.base_url, path));
        if let Some(api_key) = &self.api_key {
            request.header("api-key", api_key)
        } else {
            request
        }
    }
}

#[derive(Debug, Deserialize)]
struct QdrantSearchResponse {
    result: Vec<QdrantScoredPoint>,
}

#[derive(Debug, Deserialize)]
struct QdrantScoredPoint {
    id: Value,
    score: f32,
    #[serde(default)]
    payload: Value,
}

fn point_id_to_uuid(value: &Value) -> Option<Uuid> {
    match value {
        Value::String(value) => Uuid::parse_str(value).ok(),
        Value::Object(map) => map
            .get("uuid")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok()),
        _ => None,
    }
}

fn normalize_qdrant_http_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    let Ok(mut url) = Url::parse(trimmed) else {
        return "http://127.0.0.1:6333".to_string();
    };
    if url.port() == Some(6334) {
        let _ = url.set_port(Some(6333));
    }
    url.as_str().trim_end_matches('/').to_string()
}

async fn qdrant_status_error(action: &str, status: StatusCode) -> AroError {
    AroError::RuntimeUnavailable(format!("Qdrant returned {status} while {action}"))
}

fn map_reqwest(err: reqwest::Error) -> AroError {
    AroError::RuntimeUnavailable(err.to_string())
}

#[derive(Clone)]
pub struct MemoryVectorService {
    config: VectorMemoryConfig,
    index: QdrantMemoryIndex,
    embedder: Arc<dyn EmbeddingProvider>,
}

impl MemoryVectorService {
    pub fn from_env() -> Self {
        Self::new(VectorMemoryConfig::from_env())
    }

    pub fn new(config: VectorMemoryConfig) -> Self {
        let timeout = Duration::from_millis(config.request_timeout_ms);
        let embedder: Arc<dyn EmbeddingProvider> = match config.embedding_provider {
            EmbeddingProviderKind::Mock => Arc::new(MockEmbeddingProvider::new(
                config.embedding_model.clone(),
                64,
            )),
            EmbeddingProviderKind::Ollama => Arc::new(OllamaEmbeddingProvider::new(
                config.ollama_endpoint.clone(),
                config.embedding_model.clone(),
                timeout,
            )),
        };
        let index = QdrantMemoryIndex::new(&config);
        Self {
            config,
            index,
            embedder,
        }
    }

    pub fn config(&self) -> &VectorMemoryConfig {
        &self.config
    }

    pub async fn status(&self) -> AroResult<MemoryIndexStatus> {
        if self.config.mode == VectorMemoryMode::Disabled {
            return Ok(self.status_value("disabled", None, None, None, None));
        }
        match self.embedder.embed("aro memory status probe").await {
            Ok(vector) => {
                let collection = collection_name(
                    self.embedder.provider_id(),
                    self.embedder.model_id(),
                    vector.len(),
                );
                let ensure = self
                    .index
                    .ensure_collection(&collection, vector.len())
                    .await;
                match ensure {
                    Ok(()) => {
                        let count = self
                            .index
                            .collection_count(&collection)
                            .await
                            .ok()
                            .flatten();
                        Ok(self.status_value(
                            "active",
                            Some(collection),
                            Some(vector.len()),
                            count,
                            None,
                        ))
                    }
                    Err(error) => self.handle_status_error(error),
                }
            }
            Err(error) => self.handle_status_error(error),
        }
    }

    pub async fn index_memory(
        &self,
        memory: &LongTermMemory,
        scope: &MemoryVectorScope,
    ) -> AroResult<MemoryIndexStatus> {
        if self.config.mode == VectorMemoryMode::Disabled || !memory.approved_for_recall() {
            return Ok(self.status_value("disabled", None, None, None, None));
        }
        let result = async {
            let vector = self.embedder.embed(&memory.content).await?;
            let collection = collection_name(
                self.embedder.provider_id(),
                self.embedder.model_id(),
                vector.len(),
            );
            self.index
                .ensure_collection(&collection, vector.len())
                .await?;
            self.index
                .upsert_memory(
                    &collection,
                    memory,
                    vector,
                    scope,
                    &format!(
                        "{}:{}",
                        self.embedder.provider_id(),
                        self.embedder.model_id()
                    ),
                )
                .await?;
            Ok::<_, AroError>(self.status_value("active", Some(collection), None, None, None))
        }
        .await;
        self.handle_operation_result(result)
    }

    pub async fn delete_memory(
        &self,
        memory_id: Uuid,
        scope: &MemoryVectorScope,
    ) -> AroResult<MemoryIndexStatus> {
        if self.config.mode == VectorMemoryMode::Disabled {
            return Ok(self.status_value("disabled", None, None, None, None));
        }
        let result = async {
            let vector = self.embedder.embed("aro memory delete probe").await?;
            let collection = collection_name(
                self.embedder.provider_id(),
                self.embedder.model_id(),
                vector.len(),
            );
            self.index
                .delete_memory(&collection, memory_id, scope)
                .await?;
            Ok::<_, AroError>(self.status_value(
                "active",
                Some(collection),
                Some(vector.len()),
                None,
                None,
            ))
        }
        .await;
        self.handle_operation_result(result)
    }

    pub async fn clear_scope(&self, scope: &MemoryVectorScope) -> AroResult<MemoryIndexStatus> {
        if self.config.mode == VectorMemoryMode::Disabled {
            return Ok(self.status_value("disabled", None, None, None, None));
        }
        let result = async {
            let vector = self.embedder.embed("aro memory clear probe").await?;
            let collection = collection_name(
                self.embedder.provider_id(),
                self.embedder.model_id(),
                vector.len(),
            );
            self.index.clear_scope(&collection, scope).await?;
            Ok::<_, AroError>(self.status_value(
                "active",
                Some(collection),
                Some(vector.len()),
                None,
                None,
            ))
        }
        .await;
        self.handle_operation_result(result)
    }

    pub async fn reindex(
        &self,
        memories: &[LongTermMemory],
        scope: &MemoryVectorScope,
    ) -> AroResult<MemoryReindexReport> {
        if self.config.mode == VectorMemoryMode::Disabled {
            return Ok(MemoryReindexReport {
                status: self.status_value("disabled", None, None, None, None),
                indexed_count: 0,
                skipped_count: memories.len(),
            });
        }

        let mut indexed_count = 0;
        let mut skipped_count = 0;
        let mut last_status = None;
        for memory in memories {
            if !memory.approved_for_recall() {
                skipped_count += 1;
                continue;
            }
            match self.index_memory(memory, scope).await {
                Ok(status) if status.state == "active" => {
                    indexed_count += 1;
                    last_status = Some(status);
                }
                Ok(status) => {
                    skipped_count += 1;
                    last_status = Some(status);
                    if self.config.mode == VectorMemoryMode::Required {
                        break;
                    }
                }
                Err(error) => {
                    if self.config.mode == VectorMemoryMode::Required {
                        return Err(error);
                    }
                    skipped_count += 1;
                    last_status = Some(self.degraded_status(error.to_string()));
                    break;
                }
            }
        }

        Ok(MemoryReindexReport {
            status: last_status.unwrap_or_else(|| {
                self.status_value(
                    "active",
                    None,
                    None,
                    None,
                    Some("no approved memories".to_string()),
                )
            }),
            indexed_count,
            skipped_count,
        })
    }

    pub async fn search(
        &self,
        query: &str,
        scope: &MemoryVectorScope,
        limit: usize,
    ) -> AroResult<Vec<VectorMemoryHit>> {
        let limit = limit.clamp(1, 50);
        if self.config.mode == VectorMemoryMode::Disabled || query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let vector = match self.embedder.embed(query).await {
            Ok(v) => v,
            Err(e) if self.config.mode == VectorMemoryMode::Required => return Err(e),
            Err(_) => return Ok(Vec::new()),
        };
        let dimension = vector.len();
        let collection = collection_name(
            self.embedder.provider_id(),
            self.embedder.model_id(),
            dimension,
        );
        if let Err(e) = self.index.ensure_collection(&collection, dimension).await {
            if self.config.mode == VectorMemoryMode::Required {
                return Err(e);
            }
            return Ok(Vec::new());
        }
        let hits = match self.index.search(&collection, vector, scope, limit).await {
            Ok(h) => h,
            Err(e) if self.config.mode == VectorMemoryMode::Required => return Err(e),
            Err(_) => Vec::new(),
        };
        Ok(hits)
    }

    pub async fn retrieve(
        &self,
        query: &str,
        all_memories: Vec<LongTermMemory>,
        lexical_memories: Vec<LongTermMemory>,
        scope: &MemoryVectorScope,
        limit: usize,
    ) -> AroResult<MemoryRetrievalResult> {
        let limit = limit.clamp(1, 50);
        let mut vector_hits = Vec::new();
        let mut vector_status = self.status_value("disabled", None, None, None, None);
        if self.config.mode != VectorMemoryMode::Disabled && !query.trim().is_empty() {
            let result = async {
                let vector = self.embedder.embed(query).await?;
                let dimension = vector.len();
                let collection = collection_name(
                    self.embedder.provider_id(),
                    self.embedder.model_id(),
                    dimension,
                );
                self.index.ensure_collection(&collection, dimension).await?;
                let hits = self
                    .index
                    .search(&collection, vector, scope, limit.saturating_mul(3).max(8))
                    .await?;
                Ok::<_, AroError>((collection, hits, dimension))
            }
            .await;
            match result {
                Ok((collection, hits, dimension)) => {
                    vector_hits = hits;
                    vector_status =
                        self.status_value("active", Some(collection), Some(dimension), None, None);
                }
                Err(error) if self.config.mode == VectorMemoryMode::Required => return Err(error),
                Err(error) => {
                    vector_status = self.degraded_status(error.to_string());
                }
            }
        }

        let memories = merge_memory_results(all_memories, lexical_memories, vector_hits, limit);
        Ok(MemoryRetrievalResult {
            memories,
            vector_status,
        })
    }

    fn handle_status_error(&self, error: AroError) -> AroResult<MemoryIndexStatus> {
        if self.config.mode == VectorMemoryMode::Required {
            Err(error)
        } else {
            Ok(self.degraded_status(error.to_string()))
        }
    }

    fn handle_operation_result(
        &self,
        result: AroResult<MemoryIndexStatus>,
    ) -> AroResult<MemoryIndexStatus> {
        match result {
            Ok(status) => Ok(status),
            Err(error) if self.config.mode == VectorMemoryMode::Required => Err(error),
            Err(error) => Ok(self.degraded_status(error.to_string())),
        }
    }

    fn degraded_status(&self, message: String) -> MemoryIndexStatus {
        self.status_value("degraded", None, None, None, Some(message))
    }

    fn status_value(
        &self,
        state: impl Into<String>,
        collection_name: Option<String>,
        dimension: Option<usize>,
        indexed_count: Option<u64>,
        message: Option<String>,
    ) -> MemoryIndexStatus {
        MemoryIndexStatus {
            mode: self.config.mode,
            state: state.into(),
            qdrant_url: self.index.base_url().to_string(),
            collection_name,
            embedding_provider: self.embedder.provider_id().to_string(),
            embedding_model: self.embedder.model_id().to_string(),
            dimension,
            indexed_count,
            message,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MemoryRetrievalResult {
    pub memories: Vec<LongTermMemory>,
    pub vector_status: MemoryIndexStatus,
}

pub fn memories_to_context_sources(memories: Vec<LongTermMemory>) -> Vec<ContextSource> {
    memories.into_iter().map(memory_to_context_source).collect()
}

pub fn memory_to_context_source(memory: LongTermMemory) -> ContextSource {
    let score = if memory.pinned {
        1.0
    } else {
        memory.salience.clamp(0.0, 1.0)
    };
    ContextSource {
        id: format!("memory:{}", memory.id),
        kind: "memory".to_string(),
        title: format!("Memory / {}", memory.category),
        excerpt: compact_excerpt(&memory.content, 500),
        uri: Some(format!("memory://{}", memory.id)),
        score,
        created_at: Some(memory.created_at),
    }
}

pub fn merge_memory_results(
    all_memories: Vec<LongTermMemory>,
    lexical_memories: Vec<LongTermMemory>,
    vector_hits: Vec<VectorMemoryHit>,
    limit: usize,
) -> Vec<LongTermMemory> {
    let mut all_by_id = all_memories
        .into_iter()
        .filter(LongTermMemory::approved_for_recall)
        .map(|memory| (memory.id, memory))
        .collect::<HashMap<_, _>>();

    // Also include any approved lexical memories if not already present
    for memory in &lexical_memories {
        if memory.approved_for_recall() {
            all_by_id.entry(memory.id).or_insert_with(|| memory.clone());
        }
    }

    let mut ordered = Vec::new();

    // 1. Pinned memories have highest priority and permanent immunity
    let mut pinned = all_by_id
        .values()
        .filter(|memory| memory.pinned)
        .cloned()
        .collect::<Vec<_>>();
    pinned.sort_by(|left, right| {
        right
            .salience
            .partial_cmp(&left.salience)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    for memory in pinned {
        all_by_id.remove(&memory.id);
        ordered.push(memory);
    }

    // 2. Perform Reciprocal Rank Fusion on remaining unpinned memories
    let fts_ids: Vec<Uuid> = lexical_memories
        .iter()
        .filter(|m| m.approved_for_recall() && all_by_id.contains_key(&m.id))
        .map(|m| m.id)
        .collect();

    let valid_vec_hits: Vec<VectorMemoryHit> = vector_hits
        .into_iter()
        .filter(|hit| all_by_id.contains_key(&hit.memory_id))
        .collect();

    let rrf_scores = reciprocal_rank_fusion(
        &fts_ids,
        &valid_vec_hits,
        DEFAULT_RRF_K,
        DEFAULT_WEIGHT_FTS,
        DEFAULT_WEIGHT_VEC,
    );

    let max_possible_rrf = (DEFAULT_WEIGHT_FTS + DEFAULT_WEIGHT_VEC) / (DEFAULT_RRF_K + 1.0);
    let now = Utc::now();

    let mut scored_unpinned: Vec<(LongTermMemory, f32)> = Vec::new();

    for fused in rrf_scores {
        if let Some(memory) = all_by_id.remove(&fused.memory_id) {
            let normalized_hybrid = if max_possible_rrf > 0.0 {
                (fused.rrf_score / max_possible_rrf).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let last_used = memory.last_used_at.unwrap_or(memory.created_at);
            let recency = compute_recency_decay(last_used, now, memory.pinned);
            let utility = compute_memory_utility(
                normalized_hybrid,
                memory.salience,
                recency,
                memory.recall_count,
            );
            scored_unpinned.push((memory, utility));
        }
    }

    // Any remaining memories not in FTS or Vec hits get baseline utility
    for (_, memory) in all_by_id {
        let last_used = memory.last_used_at.unwrap_or(memory.created_at);
        let recency = compute_recency_decay(last_used, now, memory.pinned);
        let utility = compute_memory_utility(
            0.0,
            memory.salience,
            recency,
            memory.recall_count,
        );
        scored_unpinned.push((memory, utility));
    }

    // Sort unpinned by dynamic utility descending
    scored_unpinned.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.0.salience.partial_cmp(&left.0.salience).unwrap_or(std::cmp::Ordering::Equal))
            .then_with(|| right.0.updated_at.cmp(&left.0.updated_at))
            .then_with(|| left.0.id.cmp(&right.0.id))
    });

    for (memory, _) in scored_unpinned {
        ordered.push(memory);
    }

    diversify_memories(ordered, limit)
}

fn diversify_memories(memories: Vec<LongTermMemory>, limit: usize) -> Vec<LongTermMemory> {
    let mut selected = Vec::new();
    let mut selected_ids = Vec::new();
    let mut selected_categories = Vec::new();
    let mut remaining = Vec::new();

    for memory in memories {
        if memory.pinned && selected.len() < limit {
            selected_categories.push(memory.category.clone());
            selected_ids.push(memory.id);
            selected.push(memory);
        } else {
            remaining.push(memory);
        }
    }

    for memory in &remaining {
        if selected.len() >= limit {
            break;
        }
        if selected_ids.contains(&memory.id) || selected_categories.contains(&memory.category) {
            continue;
        }
        selected_categories.push(memory.category.clone());
        selected_ids.push(memory.id);
        selected.push(memory.clone());
    }

    for memory in remaining {
        if selected.len() >= limit {
            break;
        }
        if selected_ids.contains(&memory.id) {
            continue;
        }
        selected_ids.push(memory.id);
        selected.push(memory);
    }

    selected
}

fn memory_payload(
    memory: &LongTermMemory,
    scope: &MemoryVectorScope,
    embedding_model: &str,
) -> Value {
    json!({
        "namespace": scope.namespace.clone(),
        "organizationId": scope.organization_id.map(|id| id.to_string()),
        "ownerUserId": scope.owner_user_id.map(|id| id.to_string()),
        "memoryId": memory.id.to_string(),
        "clientId": memory.client_id.clone(),
        "category": memory.category.clone(),
        "scope": memory.scope.clone(),
        "status": memory.status.clone(),
        "pinned": memory.pinned,
        "salience": memory.salience,
        "contentHash": content_hash(&memory.content),
        "updatedAt": memory.updated_at.to_rfc3339(),
        "embeddingModel": embedding_model,
    })
}

fn scope_filter(scope: &MemoryVectorScope) -> Value {
    let mut must = vec![match_condition("namespace", &scope.namespace)];
    if let Some(organization_id) = scope.organization_id {
        must.push(match_condition(
            "organizationId",
            &organization_id.to_string(),
        ));
    }
    if let Some(owner_user_id) = scope.owner_user_id {
        must.push(match_condition("ownerUserId", &owner_user_id.to_string()));
    }
    json!({ "must": must })
}

fn search_filter(scope: &MemoryVectorScope) -> Value {
    let mut must = scope_filter(scope)
        .get("must")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    must.push(match_condition("status", MEMORY_STATUS_APPROVED));
    json!({ "must": must })
}

fn match_condition(key: &str, value: &str) -> Value {
    json!({
        "key": key,
        "match": { "value": value }
    })
}

pub fn collection_name(provider: &str, model: &str, dimension: usize) -> String {
    format!(
        "aro_memories_v1_{}_{}_{}",
        slugify(provider),
        slugify(model),
        dimension
    )
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if slug.is_empty() {
        "default".to_string()
    } else {
        slug
    }
}

pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn compact_excerpt(content: &str, max_chars: usize) -> String {
    let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        format!(
            "{}...",
            normalized
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

#[allow(dead_code)]
fn parse_datetime(value: Option<&str>) -> Option<DateTime<Utc>> {
    value
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(content: &str, category: &str, pinned: bool) -> LongTermMemory {
        let mut memory = LongTermMemory::new(content, None);
        memory.category = category.to_string();
        memory.pinned = pinned;
        memory
    }

    #[tokio::test]
    async fn mock_embeddings_are_deterministic() {
        let provider = MockEmbeddingProvider::new("mock-embed", 16);
        let left = provider.embed("hello semantic memory").await.unwrap();
        let right = provider.embed("hello semantic memory").await.unwrap();
        assert_eq!(left, right);
        assert_eq!(left.len(), 16);
    }

    #[test]
    fn qdrant_payload_does_not_expose_content() {
        let memory = memory("secret project detail", "technical", false);
        let payload = memory_payload(&memory, &MemoryVectorScope::local(), "mock:model");
        let json = serde_json::to_string(&payload).unwrap();
        assert!(!json.contains("secret project detail"));
        assert!(json.contains("contentHash"));
    }

    #[test]
    fn merge_prefers_pinned_then_vector_then_lexical_with_diversity() {
        let pinned = memory("always reply in french", "preference", true);
        let vector = memory("ollama embedding model", "technical", false);
        let lexical = memory("qdrant docker service", "system", false);
        let merged = merge_memory_results(
            vec![pinned.clone(), vector.clone(), lexical.clone()],
            vec![lexical.clone()],
            vec![VectorMemoryHit {
                memory_id: vector.id,
                score: 0.95,
            }],
            3,
        );
        assert_eq!(merged[0].id, pinned.id);
        assert_eq!(merged[1].id, vector.id);
        assert_eq!(merged[2].id, lexical.id);
    }

    #[test]
    fn test_rrf_multimodal_consensus_and_unimodal() {
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();
        let id_d = Uuid::new_v4();

        let fts_hits = vec![id_a, id_d, id_c];
        let vec_hits = vec![
            VectorMemoryHit {
                memory_id: id_a,
                score: 0.99,
            },
            VectorMemoryHit {
                memory_id: id_b,
                score: 0.95,
            },
            VectorMemoryHit {
                memory_id: id_d,
                score: 0.80,
            },
        ];

        let fused = reciprocal_rank_fusion(&fts_hits, &vec_hits, 60.0, 0.40, 0.60);

        assert_eq!(fused[0].memory_id, id_a);
        assert_eq!(fused[0].fts_rank, Some(1));
        assert_eq!(fused[0].vec_rank, Some(1));

        let score_a = (0.40 / 61.0) + (0.60 / 61.0);
        assert!((fused[0].rrf_score - score_a).abs() < 1e-6);

        // Check clamping on k and weights
        let clamped = reciprocal_rank_fusion(&fts_hits, &vec_hits, -10.0, -0.5, 0.5);
        assert!(!clamped.is_empty());
        assert!(!clamped[0].rrf_score.is_nan());
    }

    #[test]
    fn test_rrf_empty_inputs() {
        let empty_vec: Vec<VectorMemoryHit> = vec![];
        let empty_fts: Vec<Uuid> = vec![];
        let fused = reciprocal_rank_fusion(&empty_fts, &empty_vec, 60.0, 0.40, 0.60);
        assert!(fused.is_empty());
    }


    #[tokio::test]
    #[ignore = "requires local Qdrant on 127.0.0.1:6333/6334"]
    async fn qdrant_local_integration_round_trips_without_raw_content() {
        let mut config = VectorMemoryConfig::from_env();
        config.mode = VectorMemoryMode::Required;
        config.embedding_provider = EmbeddingProviderKind::Mock;
        config.embedding_model = "integration-test".to_string();

        let index = QdrantMemoryIndex::new(&config);
        let embedder = MockEmbeddingProvider::new("integration-test", 16);
        let vector = embedder.embed("semantic qdrant memory").await.unwrap();
        let collection = collection_name("mock", "integration-test", vector.len());
        index
            .ensure_collection(&collection, vector.len())
            .await
            .unwrap();

        let scope = MemoryVectorScope {
            namespace: format!("integration-{}", Uuid::new_v4()),
            organization_id: None,
            owner_user_id: None,
        };
        let memory = memory(
            "semantic qdrant memory content should stay private",
            "technical",
            false,
        );
        index
            .upsert_memory(
                &collection,
                &memory,
                vector,
                &scope,
                "mock:integration-test",
            )
            .await
            .unwrap();

        let mut scroll = None;
        for _ in 0..40 {
            let value = index
                .request(
                    reqwest::Method::POST,
                    &format!("/collections/{collection}/points/scroll"),
                )
                .json(&json!({
                    "filter": scope_filter(&scope),
                    "limit": 1,
                    "with_payload": true,
                    "with_vector": false
                }))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap();
            if value
                .pointer("/result/points")
                .and_then(Value::as_array)
                .is_some_and(|points| !points.is_empty())
            {
                scroll = Some(value);
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let scroll = scroll.expect("qdrant point should become visible");
        let payload_json = serde_json::to_string(&scroll).unwrap();
        assert!(!payload_json.contains("semantic qdrant memory content"));
        assert!(payload_json.contains("contentHash"));

        let hits = index
            .search(
                &collection,
                embedder.embed("semantic memory lookup").await.unwrap(),
                &scope,
                5,
            )
            .await
            .unwrap();
        assert!(hits.iter().any(|hit| hit.memory_id == memory.id));

        index
            .delete_memory(&collection, memory.id, &scope)
            .await
            .unwrap();
        let hits_after_delete = index
            .search(
                &collection,
                embedder.embed("semantic memory lookup").await.unwrap(),
                &scope,
                5,
            )
            .await
            .unwrap();
        assert!(!hits_after_delete
            .iter()
            .any(|hit| hit.memory_id == memory.id));
    }
}
