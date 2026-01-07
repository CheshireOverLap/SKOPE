//! SKOPE UI Asset
//!
//! `.ui.ron` 파일 형식을 위한 UiAsset 구조체.
//! 스키마 버전 관리와 에셋 메타데이터를 포함.

use serde::{Deserialize, Serialize};
use crate::types::Widget;

/// 현재 스키마 버전
pub const CURRENT_SCHEMA: &str = "skope/ui/v1";

/// UI 에셋 - `.ui.ron` 파일의 루트 구조체
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiAsset {
    /// 스키마 버전 (예: "skope/ui/v1")
    #[serde(default = "default_schema")]
    pub schema: String,

    /// 에셋 이름
    pub name: String,

    /// 에셋 설명 (선택적)
    #[serde(default)]
    pub description: Option<String>,

    /// 기준 해상도 (width, height)
    #[serde(default = "default_resolution")]
    pub reference_resolution: (f32, f32),

    /// 루트 위젯
    pub root: Widget,

    /// 메타데이터
    #[serde(default)]
    pub metadata: UiAssetMetadata,
}

fn default_schema() -> String {
    CURRENT_SCHEMA.to_string()
}

fn default_resolution() -> (f32, f32) {
    (1920.0, 1080.0)
}

/// UI 에셋 메타데이터
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UiAssetMetadata {
    /// 작성자
    #[serde(default)]
    pub author: Option<String>,

    /// 생성일 (ISO 8601)
    #[serde(default)]
    pub created: Option<String>,

    /// 수정일 (ISO 8601)
    #[serde(default)]
    pub modified: Option<String>,

    /// 태그
    #[serde(default)]
    pub tags: Vec<String>,

    /// 버전 (유저 정의)
    #[serde(default)]
    pub version: Option<String>,

    /// AI 협업 노트 (프로젝트 레벨)
    #[serde(default)]
    pub ai_notes: Vec<AiNote>,
}

/// AI 협업 노트
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiNote {
    /// 타임스탬프
    pub timestamp: String,

    /// 노트 내용
    pub content: String,

    /// 노트 타입 (context, decision, todo 등)
    #[serde(default = "default_note_type")]
    pub note_type: String,
}

fn default_note_type() -> String {
    "context".to_string()
}

impl Default for UiAsset {
    fn default() -> Self {
        Self {
            schema: CURRENT_SCHEMA.to_string(),
            name: "Untitled".to_string(),
            description: None,
            reference_resolution: (1920.0, 1080.0),
            root: Widget::default(),
            metadata: UiAssetMetadata::default(),
        }
    }
}

impl UiAsset {
    /// 새 UI 에셋 생성
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// RON 문자열에서 로드
    pub fn from_ron(ron_str: &str) -> Result<Self, UiAssetError> {
        ron::from_str(ron_str).map_err(|e| UiAssetError::ParseError(e.to_string()))
    }

    /// RON 문자열로 직렬화
    pub fn to_ron(&self) -> Result<String, UiAssetError> {
        let config = ron::ser::PrettyConfig::new()
            .depth_limit(10)
            .indentor("    ".to_string());
        ron::ser::to_string_pretty(self, config)
            .map_err(|e| UiAssetError::SerializeError(e.to_string()))
    }

    /// 파일에서 로드
    pub fn load<P: AsRef<std::path::Path>>(path: P) -> Result<Self, UiAssetError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| UiAssetError::IoError(e.to_string()))?;
        Self::from_ron(&content)
    }

    /// 파일에 저장
    pub fn save<P: AsRef<std::path::Path>>(&self, path: P) -> Result<(), UiAssetError> {
        let ron_str = self.to_ron()?;
        std::fs::write(path.as_ref(), ron_str)
            .map_err(|e| UiAssetError::IoError(e.to_string()))
    }

    /// 스키마 버전 확인
    pub fn is_compatible(&self) -> bool {
        self.schema.starts_with("skope/ui/")
    }

    /// 스키마 버전 가져오기
    pub fn schema_version(&self) -> Option<u32> {
        self.schema
            .strip_prefix("skope/ui/v")
            .and_then(|v| v.parse().ok())
    }

    /// 메타데이터 수정일 업데이트
    pub fn touch(&mut self) {
        use std::time::SystemTime;
        if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            // ISO 8601 간단 형식
            let secs = duration.as_secs();
            let days = secs / 86400;
            let years = 1970 + days / 365;
            let remaining_days = days % 365;
            let month = remaining_days / 30 + 1;
            let day = remaining_days % 30 + 1;
            self.metadata.modified = Some(format!("{:04}-{:02}-{:02}", years, month, day));
        }
    }

    /// AI 노트 추가
    pub fn add_ai_note(&mut self, content: impl Into<String>, note_type: impl Into<String>) {
        use std::time::SystemTime;
        let timestamp = if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            format!("{}", duration.as_secs())
        } else {
            "0".to_string()
        };

        self.metadata.ai_notes.push(AiNote {
            timestamp,
            content: content.into(),
            note_type: note_type.into(),
        });
    }
}

/// UI 에셋 에러
#[derive(Debug)]
pub enum UiAssetError {
    ParseError(String),
    SerializeError(String),
    IoError(String),
    SchemaError(String),
}

impl std::fmt::Display for UiAssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiAssetError::ParseError(s) => write!(f, "Parse error: {}", s),
            UiAssetError::SerializeError(s) => write!(f, "Serialize error: {}", s),
            UiAssetError::IoError(s) => write!(f, "IO error: {}", s),
            UiAssetError::SchemaError(s) => write!(f, "Schema error: {}", s),
        }
    }
}

impl std::error::Error for UiAssetError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_asset() {
        let asset = UiAsset::default();
        assert_eq!(asset.schema, CURRENT_SCHEMA);
        assert_eq!(asset.name, "Untitled");
        assert!(asset.is_compatible());
        assert_eq!(asset.schema_version(), Some(1));
    }

    #[test]
    fn test_ron_serialization() {
        let asset = UiAsset::new("TestUI");
        let ron = asset.to_ron().unwrap();
        assert!(ron.contains("skope/ui/v1"));
        assert!(ron.contains("TestUI"));

        let loaded = UiAsset::from_ron(&ron).unwrap();
        assert_eq!(loaded.name, "TestUI");
    }
}
