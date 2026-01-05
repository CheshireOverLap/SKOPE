//! SKOPE MCP Server
//!
//! Model Context Protocol (MCP) 서버 구현
//! AI 어시스턴트와 에디터 간의 통신을 담당
//!
//! ## 주요 기능
//! - ECS 조회/수정 도구
//! - 씬 저장/로드 도구
//! - 코드 생성 도구
//! - 프로젝트 메모리/TODO 관리

pub mod protocol;
pub mod server;
pub mod tools;
pub mod resources;

pub use protocol::{McpRequest, McpResponse, McpError};
pub use server::McpServer;
pub use tools::{Tool, ToolResult};
pub use resources::{Resource, ResourceResult};

/// MCP 서버 설정
#[derive(Debug, Clone)]
pub struct McpConfig {
    /// WebSocket 포트 (None이면 stdio 모드)
    pub websocket_port: Option<u16>,
    /// 서버 이름
    pub server_name: String,
    /// 서버 버전
    pub server_version: String,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            websocket_port: Some(9876),
            server_name: "skope-mcp".to_string(),
            server_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// AI 채팅 메시지
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessage {
    /// 메시지 역할 (user, assistant, system)
    pub role: String,
    /// 메시지 내용
    pub content: String,
    /// 타임스탬프
    pub timestamp: f64,
    /// 도구 호출 (assistant만)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

/// 도구 호출 정보
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCall {
    /// 도구 이름
    pub name: String,
    /// 도구 인자
    pub arguments: serde_json::Value,
    /// 실행 결과
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 성공 여부
    pub success: bool,
}

/// TODO 항목
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    /// 고유 ID
    pub id: String,
    /// 내용
    pub content: String,
    /// 상태
    pub status: TodoStatus,
    /// 생성 시간
    pub created_at: f64,
    /// 완료 시간
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<f64>,
}

/// TODO 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// 프로젝트 노트
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Note {
    /// 고유 ID
    pub id: String,
    /// 제목
    pub title: String,
    /// 내용
    pub content: String,
    /// 태그
    #[serde(default)]
    pub tags: Vec<String>,
    /// 생성 시간
    pub created_at: f64,
    /// 수정 시간
    pub updated_at: f64,
}
