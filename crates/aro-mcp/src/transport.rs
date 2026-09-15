use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;

use crate::protocol::{JsonRpcRequest, JsonRpcResponse};

#[async_trait]
pub trait McpTransport: Send + Sync {
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<JsonRpcResponse>;
    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()>;
    async fn close(&self) -> Result<()>;
}

pub struct StdioTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    _child: Arc<Mutex<Child>>,
    request_id: AtomicI64,
}

impl StdioTransport {
    pub async fn spawn(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&PathBuf>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.envs(env);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        let mut child = cmd.spawn().with_context(|| {
            format!(
                "failed to spawn MCP server process '{}' with args {:?}",
                command, args
            )
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to open child process stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to open child process stdout"))?;

        Ok(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: Arc::new(Mutex::new(BufReader::new(stdout))),
            _child: Arc::new(Mutex::new(child)),
            request_id: AtomicI64::new(1),
        })
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<JsonRpcResponse> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);
        let mut serialized = serde_json::to_string(&request)?;
        serialized.push('\n');

        // Write to stdin
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(serialized.as_bytes()).await?;
            stdin.flush().await?;
        }

        // Read response line with timeout
        let response_line = tokio::time::timeout(Duration::from_secs(30), async {
            let mut stdout = self.stdout.lock().await;
            let mut line = String::new();
            loop {
                line.clear();
                let bytes = stdout.read_line(&mut line).await?;
                if bytes == 0 {
                    return Err(anyhow!(
                        "MCP server process closed standard output unexpectedly"
                    ));
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                // Try parse as JSON-RPC response
                if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                    if resp.id == Some(Value::from(id)) {
                        return Ok(resp);
                    }
                }
            }
        })
        .await
        .map_err(|_| anyhow!("timeout waiting for MCP server response to '{}'", method))??;

        Ok(response_line)
    }

    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notif = JsonRpcRequest::notification(method, params);
        let mut serialized = serde_json::to_string(&notif)?;
        serialized.push('\n');

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(serialized.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        let mut child = self._child.lock().await;
        let _ = child.kill().await;
        Ok(())
    }
}

pub struct HttpTransport {
    client: reqwest::Client,
    url: String,
    headers: HashMap<String, String>,
    request_id: AtomicI64,
}

impl HttpTransport {
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            url: url.into(),
            headers,
            request_id: AtomicI64::new(1),
        }
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<JsonRpcResponse> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        let mut req_builder = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }

        let resp =
            req_builder.json(&request).send().await.with_context(|| {
                format!("failed to send HTTP JSON-RPC request to '{}'", self.url)
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!("MCP HTTP error status {}: {}", status, body));
        }

        let json_resp = resp.json::<JsonRpcResponse>().await.with_context(|| {
            format!("failed to parse MCP JSON-RPC response from '{}'", self.url)
        })?;

        Ok(json_resp)
    }

    async fn send_notification(&self, method: &str, params: Option<Value>) -> Result<()> {
        let notif = JsonRpcRequest::notification(method, params);
        let mut req_builder = self
            .client
            .post(&self.url)
            .header("Content-Type", "application/json");

        for (k, v) in &self.headers {
            req_builder = req_builder.header(k, v);
        }

        let _ = req_builder.json(&notif).send().await;
        Ok(())
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }
}
