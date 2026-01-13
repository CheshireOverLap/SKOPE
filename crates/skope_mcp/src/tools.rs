//! MCP Tools
//!
//! ECS 조회/수정, 씬 작업, 코드 생성 등의 도구

pub mod ecs;
pub mod scene;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use serde_json::Value;

use crate::protocol::{McpError, ToolDefinition};

/// 도구 실행 결과
pub type ToolResult = Result<Value, McpError>;

/// 도구 실행 함수 타입
pub type ToolFn = Box<dyn Fn(Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>> + Send + Sync>;

/// 도구 trait
pub trait Tool: Send + Sync {
    /// 도구 이름
    fn name(&self) -> &'static str;

    /// 도구 설명
    fn description(&self) -> &'static str;

    /// 입력 스키마
    fn input_schema(&self) -> Value;

    /// 도구 실행
    fn call(&self, args: Value) -> Pin<Box<dyn Future<Output = ToolResult> + Send>>;

    /// ToolDefinition으로 변환
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: self.description().to_string(),
            input_schema: self.input_schema(),
        }
    }
}

/// 도구 레지스트리
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    /// 새 레지스트리 생성
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };

        // 기본 도구 등록
        registry.register(Box::new(ecs::EcsQueryTool));
        registry.register(Box::new(ecs::EcsModifyTool));
        registry.register(Box::new(ecs::EntitySpawnTool));
        registry.register(Box::new(ecs::EntityDeleteTool));
        registry.register(Box::new(scene::SceneSaveTool));
        registry.register(Box::new(scene::SceneLoadTool));
        registry.register(Box::new(scene::SceneListTool));

        registry
    }

    /// 도구 등록
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// 도구 목록 조회
    pub fn list(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// 도구 호출
    pub async fn call(&self, name: &str, args: Value) -> ToolResult {
        let tool = self.tools.get(name)
            .ok_or_else(|| McpError::method_not_found(format!("Tool not found: {}", name)))?;

        tool.call(args).await
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
