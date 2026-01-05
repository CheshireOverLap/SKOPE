//! MCP Resources
//!
//! 메모리, 프로젝트 컨텍스트, TODO 리소스

pub mod memory;
pub mod project;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use crate::protocol::{McpError, ResourceContent, ResourceDefinition};

/// 리소스 읽기 결과
pub type ResourceResult = Result<ResourceContent, McpError>;

/// 리소스 trait
pub trait Resource: Send + Sync {
    /// 리소스 URI
    fn uri(&self) -> &'static str;

    /// 리소스 이름
    fn name(&self) -> &'static str;

    /// 리소스 설명
    fn description(&self) -> &'static str;

    /// MIME 타입
    fn mime_type(&self) -> &'static str {
        "application/json"
    }

    /// 리소스 읽기
    fn read(&self) -> Pin<Box<dyn Future<Output = ResourceResult> + Send>>;

    /// ResourceDefinition으로 변환
    fn definition(&self) -> ResourceDefinition {
        ResourceDefinition {
            uri: self.uri().to_string(),
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            mime_type: Some(self.mime_type().to_string()),
        }
    }
}

/// 리소스 레지스트리
pub struct ResourceRegistry {
    resources: HashMap<String, Box<dyn Resource>>,
    subscriptions: HashMap<String, bool>,
}

impl ResourceRegistry {
    /// 새 레지스트리 생성
    pub fn new() -> Self {
        let mut registry = Self {
            resources: HashMap::new(),
            subscriptions: HashMap::new(),
        };

        // 기본 리소스 등록
        registry.register(Box::new(memory::MemoryResource::new()));
        registry.register(Box::new(memory::TodoResource::new()));
        registry.register(Box::new(project::ProjectContextResource::new()));
        registry.register(Box::new(project::SceneResource::new()));

        registry
    }

    /// 리소스 등록
    pub fn register(&mut self, resource: Box<dyn Resource>) {
        self.resources.insert(resource.uri().to_string(), resource);
    }

    /// 리소스 목록 조회
    pub fn list(&self) -> Vec<ResourceDefinition> {
        self.resources.values().map(|r| r.definition()).collect()
    }

    /// 리소스 읽기
    pub async fn read(&self, uri: &str) -> ResourceResult {
        let resource = self.resources.get(uri)
            .ok_or_else(|| McpError::invalid_params(format!("Resource not found: {}", uri)))?;

        resource.read().await
    }

    /// 구독
    pub fn subscribe(&mut self, uri: &str) -> Result<(), McpError> {
        if !self.resources.contains_key(uri) {
            return Err(McpError::invalid_params(format!("Resource not found: {}", uri)));
        }
        self.subscriptions.insert(uri.to_string(), true);
        Ok(())
    }

    /// 구독 해제
    pub fn unsubscribe(&mut self, uri: &str) -> Result<(), McpError> {
        self.subscriptions.remove(uri);
        Ok(())
    }

    /// 구독 여부 확인
    pub fn is_subscribed(&self, uri: &str) -> bool {
        self.subscriptions.get(uri).copied().unwrap_or(false)
    }
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}
