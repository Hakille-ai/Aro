use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::protocol::{
    CallToolResult, ImplementationInfo, InitializeParams, InitializeResult, ListToolsResult,
    McpTool, LATEST_PROTOCOL_VERSION,
};
use crate::transport::{HttpTransport, McpTransport, StdioTransport};

#[derive(Clone)]
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    server_info: Option<ImplementationInfo>,
}

impl McpClient {
    pub async fn connect_stdio(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
        cwd: Option<&PathBuf>,
    ) -> Result<Self> {
        let transport = StdioTransport::spawn(command, args, env, cwd).await?;
        let mut client = Self {
            transport: Arc::new(transport),
            server_info: None,
        };
        client.initialize().await?;
        Ok(client)
    }

    pub async fn connect_http(
        url: impl Into<String>,
        headers: HashMap<String, String>,
    ) -> Result<Self> {
        let transport = HttpTransport::new(url, headers);
        let mut client = Self {
            transport: Arc::new(transport),
            server_info: None,
        };
        client.initialize().await?;
        Ok(client)
    }

    pub fn server_info(&self) -> Option<&ImplementationInfo> {
        self.server_info.as_ref()
    }

    pub async fn initialize(&mut self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: json!({
                "roots": { "listChanged": false },
                "sampling": {}
            }),
            client_info: ImplementationInfo {
                name: "aro-agent".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let response = self
            .transport
            .send_request("initialize", Some(serde_json::to_value(&params)?))
            .await?;

        if let Some(err) = response.error {
            return Err(anyhow!("MCP initialize error: {}", err));
        }

        let result_value = response
            .result
            .ok_or_else(|| anyhow!("MCP initialize response missing result"))?;

        let init_result: InitializeResult = serde_json::from_value(result_value)?;
        self.server_info = init_result.server_info.clone();

        // Send initialized notification
        let _ = self
            .transport
            .send_notification("notifications/initialized", None)
            .await;

        Ok(init_result)
    }

    pub async fn ping(&self) -> Result<()> {
        let resp = self.transport.send_request("ping", None).await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("MCP ping error: {}", err));
        }
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let resp = self
            .transport
            .send_request("tools/list", Some(json!({})))
            .await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("MCP tools/list error: {}", err));
        }
        let result_value = resp
            .result
            .ok_or_else(|| anyhow!("MCP tools/list missing result"))?;

        let list_res: ListToolsResult = serde_json::from_value(result_value)?;
        Ok(list_res.tools)
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<CallToolResult> {
        let params = json!({
            "name": name,
            "arguments": arguments,
        });

        let resp = self
            .transport
            .send_request("tools/call", Some(params))
            .await?;
        if let Some(err) = resp.error {
            return Err(anyhow!("MCP tools/call error: {}", err));
        }

        let result_value = resp
            .result
            .ok_or_else(|| anyhow!("MCP tools/call missing result"))?;

        let call_res: CallToolResult = serde_json::from_value(result_value)?;
        Ok(call_res)
    }

    pub async fn close(&self) -> Result<()> {
        self.transport.close().await
    }
}
