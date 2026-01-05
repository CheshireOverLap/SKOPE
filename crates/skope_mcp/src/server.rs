//! MCP Server Implementation
//!
//! WebSocket/stdio 기반 MCP 서버

use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::Value;

use crate::protocol::*;
use crate::tools::ToolRegistry;
use crate::resources::ResourceRegistry;
use crate::McpConfig;

/// MCP 서버
pub struct McpServer {
    /// 서버 설정
    config: McpConfig,
    /// 도구 레지스트리
    tools: Arc<RwLock<ToolRegistry>>,
    /// 리소스 레지스트리
    resources: Arc<RwLock<ResourceRegistry>>,
    /// 초기화 완료 여부
    initialized: Arc<RwLock<bool>>,
}

impl McpServer {
    /// 새 MCP 서버 생성
    pub fn new(config: McpConfig) -> Self {
        Self {
            config,
            tools: Arc::new(RwLock::new(ToolRegistry::new())),
            resources: Arc::new(RwLock::new(ResourceRegistry::new())),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// 도구 레지스트리 접근
    pub fn tools(&self) -> Arc<RwLock<ToolRegistry>> {
        Arc::clone(&self.tools)
    }

    /// 리소스 레지스트리 접근
    pub fn resources(&self) -> Arc<RwLock<ResourceRegistry>> {
        Arc::clone(&self.resources)
    }

    /// 요청 처리
    pub async fn handle_request(&self, request: Value) -> Value {
        // JSON-RPC 요청 파싱
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(|m| m.as_str());
        let params = request.get("params").cloned().unwrap_or(Value::Null);

        let result = match method {
            Some("initialize") => self.handle_initialize(params).await,
            Some("tools/list") => self.handle_list_tools().await,
            Some("tools/call") => self.handle_call_tool(params).await,
            Some("resources/list") => self.handle_list_resources().await,
            Some("resources/read") => self.handle_read_resource(params).await,
            Some("resources/subscribe") => self.handle_subscribe(params).await,
            Some("resources/unsubscribe") => self.handle_unsubscribe(params).await,
            Some("notifications/initialized") => {
                // 클라이언트 초기화 완료 알림 (응답 없음)
                return Value::Null;
            }
            Some(method) => Err(McpError::method_not_found(format!("Unknown method: {}", method))),
            None => Err(McpError::invalid_request("Missing method")),
        };

        match result {
            Ok(result) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            }),
            Err(error) => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": error.code,
                    "message": error.message,
                    "data": error.data
                }
            }),
        }
    }

    /// 초기화 요청 처리
    async fn handle_initialize(&self, params: Value) -> Result<Value, McpError> {
        let _params: InitializeParams = serde_json::from_value(params)
            .map_err(|e| McpError::invalid_params(e.to_string()))?;

        *self.initialized.write().await = true;

        let result = InitializeResult {
            protocol_version: "2024-11-05".to_string(),
            server_info: ServerInfo {
                name: self.config.server_name.clone(),
                version: self.config.server_version.clone(),
            },
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: true }),
                resources: Some(ResourcesCapability {
                    subscribe: true,
                    list_changed: true,
                }),
                prompts: None,
            },
        };

        serde_json::to_value(result).map_err(|e| McpError::internal_error(e.to_string()))
    }

    /// 도구 목록 조회
    async fn handle_list_tools(&self) -> Result<Value, McpError> {
        let tools = self.tools.read().await;
        let tool_list: Vec<ToolDefinition> = tools.list();

        Ok(serde_json::json!({
            "tools": tool_list
        }))
    }

    /// 도구 호출
    async fn handle_call_tool(&self, params: Value) -> Result<Value, McpError> {
        let call_params: CallToolParams = serde_json::from_value(params)
            .map_err(|e| McpError::invalid_params(e.to_string()))?;

        let tools = self.tools.read().await;
        let result = tools.call(&call_params.name, call_params.arguments).await?;

        Ok(serde_json::json!({
            "content": [{
                "type": "text",
                "text": serde_json::to_string_pretty(&result).unwrap_or_default()
            }]
        }))
    }

    /// 리소스 목록 조회
    async fn handle_list_resources(&self) -> Result<Value, McpError> {
        let resources = self.resources.read().await;
        let resource_list: Vec<ResourceDefinition> = resources.list();

        Ok(serde_json::json!({
            "resources": resource_list
        }))
    }

    /// 리소스 읽기
    async fn handle_read_resource(&self, params: Value) -> Result<Value, McpError> {
        let read_params: ReadResourceParams = serde_json::from_value(params)
            .map_err(|e| McpError::invalid_params(e.to_string()))?;

        let resources = self.resources.read().await;
        let content = resources.read(&read_params.uri).await?;

        Ok(serde_json::json!({
            "contents": [content]
        }))
    }

    /// 리소스 구독
    async fn handle_subscribe(&self, params: Value) -> Result<Value, McpError> {
        let sub_params: SubscribeParams = serde_json::from_value(params)
            .map_err(|e| McpError::invalid_params(e.to_string()))?;

        let mut resources = self.resources.write().await;
        resources.subscribe(&sub_params.uri)?;

        Ok(Value::Object(serde_json::Map::new()))
    }

    /// 리소스 구독 해제
    async fn handle_unsubscribe(&self, params: Value) -> Result<Value, McpError> {
        let unsub_params: UnsubscribeParams = serde_json::from_value(params)
            .map_err(|e| McpError::invalid_params(e.to_string()))?;

        let mut resources = self.resources.write().await;
        resources.unsubscribe(&unsub_params.uri)?;

        Ok(Value::Object(serde_json::Map::new()))
    }
}
