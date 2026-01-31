//! MCP Protocol Implementation
//!
//! JSON-RPC 2.0 기반 MCP 프로토콜

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP 요청
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum McpRequest {
    /// 서버 초기화
    #[serde(rename = "initialize")]
    Initialize(InitializeParams),

    /// 도구 목록 조회
    #[serde(rename = "tools/list")]
    ListTools,

    /// 도구 실행
    #[serde(rename = "tools/call")]
    CallTool(CallToolParams),

    /// 리소스 목록 조회
    #[serde(rename = "resources/list")]
    ListResources,

    /// 리소스 읽기
    #[serde(rename = "resources/read")]
    ReadResource(ReadResourceParams),

    /// 리소스 구독
    #[serde(rename = "resources/subscribe")]
    Subscribe(SubscribeParams),

    /// 리소스 구독 해제
    #[serde(rename = "resources/unsubscribe")]
    Unsubscribe(UnsubscribeParams),

    /// 프롬프트 목록 조회
    #[serde(rename = "prompts/list")]
    ListPrompts,

    /// 프롬프트 실행
    #[serde(rename = "prompts/get")]
    GetPrompt(GetPromptParams),
}

/// 초기화 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeParams {
    /// 프로토콜 버전
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    /// 클라이언트 정보
    #[serde(rename = "clientInfo")]
    pub client_info: ClientInfo,
    /// 클라이언트 기능
    pub capabilities: ClientCapabilities,
}

/// 클라이언트 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// 클라이언트 기능
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientCapabilities {
    #[serde(default)]
    pub roots: Option<RootsCapability>,
    #[serde(default)]
    pub sampling: Option<SamplingCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootsCapability {
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingCapability {}

/// 도구 호출 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallToolParams {
    /// 도구 이름
    pub name: String,
    /// 도구 인자
    #[serde(default)]
    pub arguments: Value,
}

/// 리소스 읽기 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadResourceParams {
    /// 리소스 URI
    pub uri: String,
}

/// 구독 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeParams {
    /// 리소스 URI
    pub uri: String,
}

/// 구독 해제 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnsubscribeParams {
    /// 리소스 URI
    pub uri: String,
}

/// 프롬프트 가져오기 파라미터
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetPromptParams {
    /// 프롬프트 이름
    pub name: String,
    /// 프롬프트 인자
    #[serde(default)]
    pub arguments: Value,
}

/// MCP 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpResponse {
    /// 성공 응답
    Success(SuccessResponse),
    /// 에러 응답
    Error(ErrorResponse),
}

/// 성공 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub result: Value,
}

/// 에러 응답
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub jsonrpc: String,
    pub id: Value,
    pub error: McpError,
}

/// MCP 에러
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    /// 에러 코드
    pub code: i32,
    /// 에러 메시지
    pub message: String,
    /// 추가 데이터
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpError {
    /// 파싱 에러 (-32700)
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32700,
            message: msg.into(),
            data: None,
        }
    }

    /// 잘못된 요청 (-32600)
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self {
            code: -32600,
            message: msg.into(),
            data: None,
        }
    }

    /// 메서드를 찾을 수 없음 (-32601)
    pub fn method_not_found(msg: impl Into<String>) -> Self {
        Self {
            code: -32601,
            message: msg.into(),
            data: None,
        }
    }

    /// 잘못된 파라미터 (-32602)
    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: msg.into(),
            data: None,
        }
    }

    /// 내부 에러 (-32603)
    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
            data: None,
        }
    }
}

/// 도구 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// 도구 이름
    pub name: String,
    /// 도구 설명
    pub description: String,
    /// 입력 스키마 (JSON Schema)
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// 리소스 정의
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// 리소스 URI
    pub uri: String,
    /// 리소스 이름
    pub name: String,
    /// 리소스 설명
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME 타입
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// 리소스 내용
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    /// 리소스 URI
    pub uri: String,
    /// MIME 타입
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// 텍스트 내용
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 바이너리 내용 (base64)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// 서버 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// 서버 기능
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged", default)]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcesCapability {
    #[serde(default)]
    pub subscribe: bool,
    #[serde(rename = "listChanged", default)]
    pub list_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptsCapability {
    #[serde(rename = "listChanged", default)]
    pub list_changed: bool,
}

/// 초기화 결과
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(rename = "serverInfo")]
    pub server_info: ServerInfo,
    pub capabilities: ServerCapabilities,
}
