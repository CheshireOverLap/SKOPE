//! AI Diff View 데이터 타입
//!
//! AI 변경 제안을 표현하는 데이터 구조체들

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use bevy_ecs::entity::Entity;

/// Diff 세션 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiffSessionId(pub u64);

impl DiffSessionId {
    pub fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for DiffSessionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Diff 변경 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DiffChangeId(pub u64);

impl DiffChangeId {
    pub fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

impl Default for DiffChangeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Diff 세션 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffSessionStatus {
    /// 사용자 검토 대기
    #[default]
    Pending,
    /// 일부 변경만 승인됨
    Partial,
    /// 전체 승인됨
    Accepted,
    /// 전체 거부됨
    Rejected,
}

/// AI Diff 세션
///
/// AI가 제안한 변경 사항 집합
#[derive(Debug, Clone)]
pub struct AIDiffSession {
    /// 세션 ID
    pub id: DiffSessionId,
    /// 생성 시간
    pub created_at: Instant,
    /// 원본 AI 요청 (프롬프트)
    pub prompt: String,
    /// 변경 사항 목록
    pub changes: Vec<DiffChange>,
    /// 세션 상태
    pub status: DiffSessionStatus,
    /// AI 응답 설명
    pub explanation: String,
}

impl AIDiffSession {
    /// 새 세션 생성
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            id: DiffSessionId::new(),
            created_at: Instant::now(),
            prompt: prompt.into(),
            changes: Vec::new(),
            status: DiffSessionStatus::Pending,
            explanation: String::new(),
        }
    }

    /// 변경 사항 추가
    pub fn add_change(&mut self, change: DiffChange) {
        self.changes.push(change);
    }

    /// 설명 설정
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    /// 모든 변경 승인
    pub fn accept_all(&mut self) {
        for change in &mut self.changes {
            change.set_accepted(true);
        }
        self.status = DiffSessionStatus::Accepted;
    }

    /// 모든 변경 거부
    pub fn reject_all(&mut self) {
        for change in &mut self.changes {
            change.set_accepted(false);
        }
        self.status = DiffSessionStatus::Rejected;
    }

    /// 상태 업데이트
    pub fn update_status(&mut self) {
        let accepted = self.changes.iter().filter(|c| c.is_accepted() == Some(true)).count();
        let rejected = self.changes.iter().filter(|c| c.is_accepted() == Some(false)).count();
        let total = self.changes.len();

        if accepted == total {
            self.status = DiffSessionStatus::Accepted;
        } else if rejected == total {
            self.status = DiffSessionStatus::Rejected;
        } else if accepted > 0 || rejected > 0 {
            self.status = DiffSessionStatus::Partial;
        } else {
            self.status = DiffSessionStatus::Pending;
        }
    }

    /// 승인된 변경 사항만 필터링
    pub fn accepted_changes(&self) -> impl Iterator<Item = &DiffChange> {
        self.changes.iter().filter(|c| c.is_accepted() == Some(true))
    }

    /// 대기 중인 변경 사항 수
    pub fn pending_count(&self) -> usize {
        self.changes.iter().filter(|c| c.is_accepted().is_none()).count()
    }
}

/// 개별 변경 사항
#[derive(Debug, Clone)]
pub struct DiffChange {
    /// 변경 ID
    pub id: DiffChangeId,
    /// 변경 타입
    pub change_type: DiffChangeType,
    /// 승인 상태 (None = 대기 중)
    pub accepted: Option<bool>,
    /// 변경 설명
    pub description: String,
}

impl DiffChange {
    /// 새 변경 생성
    pub fn new(change_type: DiffChangeType, description: impl Into<String>) -> Self {
        Self {
            id: DiffChangeId::new(),
            change_type,
            accepted: None,
            description: description.into(),
        }
    }

    /// 승인 상태 설정
    pub fn set_accepted(&mut self, accepted: bool) {
        self.accepted = Some(accepted);
    }

    /// 승인 상태 조회
    pub fn is_accepted(&self) -> Option<bool> {
        self.accepted
    }

    /// 승인/거부 토글
    pub fn toggle(&mut self) {
        self.accepted = match self.accepted {
            None => Some(true),
            Some(true) => Some(false),
            Some(false) => None,
        };
    }
}

/// 변경 타입
#[derive(Debug, Clone)]
pub enum DiffChangeType {
    /// 엔티티 속성 변경
    Entity {
        entity: Entity,
        entity_name: String,
        modifications: Vec<EntityModification>,
    },
    /// 컴포넌트 추가
    AddComponent {
        entity: Entity,
        entity_name: String,
        component_type: String,
        initial_values: HashMap<String, DiffValue>,
    },
    /// 컴포넌트 제거
    RemoveComponent {
        entity: Entity,
        entity_name: String,
        component_type: String,
    },
    /// 코드/스크립트 변경
    Code {
        file_path: PathBuf,
        language: String,
        hunks: Vec<DiffHunk>,
    },
    /// 에셋 생성
    CreateAsset {
        asset_path: PathBuf,
        asset_type: String,
    },
}

/// 엔티티 속성 변경
#[derive(Debug, Clone)]
pub struct EntityModification {
    /// 컴포넌트 타입 (예: "Transform")
    pub component_type: String,
    /// 필드 경로 (예: "position.x")
    pub field_path: String,
    /// 이전 값
    pub old_value: DiffValue,
    /// 새 값
    pub new_value: DiffValue,
}

/// Diff 값 (다양한 타입 지원)
#[derive(Debug, Clone)]
pub enum DiffValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Vec3 { x: f32, y: f32, z: f32 },
    Vec4 { x: f32, y: f32, z: f32, w: f32 },
    Color { r: f32, g: f32, b: f32, a: f32 },
    Array(Vec<DiffValue>),
    Object(HashMap<String, DiffValue>),
}

impl std::fmt::Display for DiffValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(v) => write!(f, "{}", v),
            Self::Int(v) => write!(f, "{}", v),
            Self::Float(v) => write!(f, "{:.3}", v),
            Self::String(v) => write!(f, "\"{}\"", v),
            Self::Vec3 { x, y, z } => write!(f, "({:.2}, {:.2}, {:.2})", x, y, z),
            Self::Vec4 { x, y, z, w } => write!(f, "({:.2}, {:.2}, {:.2}, {:.2})", x, y, z, w),
            Self::Color { r, g, b, a } => write!(f, "rgba({:.0}, {:.0}, {:.0}, {:.2})", r * 255.0, g * 255.0, b * 255.0, a),
            Self::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Self::Object(obj) => {
                write!(f, "{{")?;
                for (i, (k, v)) in obj.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

/// 코드 Diff 청크
#[derive(Debug, Clone)]
pub struct DiffHunk {
    /// 이전 파일 시작 라인
    pub old_start: usize,
    /// 이전 파일 라인 수
    pub old_count: usize,
    /// 새 파일 시작 라인
    pub new_start: usize,
    /// 새 파일 라인 수
    pub new_count: usize,
    /// Diff 라인들
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    /// 새 파일용 청크 생성 (모두 Addition)
    pub fn new_file(content: &str) -> Self {
        let lines: Vec<DiffLine> = content
            .lines()
            .enumerate()
            .map(|(i, line)| DiffLine {
                line_type: DiffLineType::Addition,
                content: line.to_string(),
                old_line_number: None,
                new_line_number: Some(i + 1),
            })
            .collect();

        Self {
            old_start: 0,
            old_count: 0,
            new_start: 1,
            new_count: lines.len(),
            lines,
        }
    }

    /// 삭제용 청크 생성 (모두 Deletion)
    pub fn delete_file(content: &str) -> Self {
        let lines: Vec<DiffLine> = content
            .lines()
            .enumerate()
            .map(|(i, line)| DiffLine {
                line_type: DiffLineType::Deletion,
                content: line.to_string(),
                old_line_number: Some(i + 1),
                new_line_number: None,
            })
            .collect();

        Self {
            old_start: 1,
            old_count: lines.len(),
            new_start: 0,
            new_count: 0,
            lines,
        }
    }
}

/// Diff 라인
#[derive(Debug, Clone)]
pub struct DiffLine {
    /// 라인 타입
    pub line_type: DiffLineType,
    /// 라인 내용
    pub content: String,
    /// 이전 파일 라인 번호
    pub old_line_number: Option<usize>,
    /// 새 파일 라인 번호
    pub new_line_number: Option<usize>,
}

/// Diff 라인 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineType {
    /// 컨텍스트 (변경 없음)
    Context,
    /// 추가
    Addition,
    /// 삭제
    Deletion,
}

impl DiffLineType {
    /// 접두사 문자
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Context => " ",
            Self::Addition => "+",
            Self::Deletion => "-",
        }
    }
}

/// Diff View 액션
#[derive(Debug, Clone)]
pub enum DiffViewAction {
    /// 없음
    None,
    /// 개별 변경 승인
    AcceptChange(DiffChangeId),
    /// 개별 변경 거부
    RejectChange(DiffChangeId),
    /// 모든 변경 승인
    AcceptAll,
    /// 모든 변경 거부
    RejectAll,
    /// 선택된 변경만 적용
    ApplySelected,
    /// 세션 닫기
    Close,
}

// ============================================================================
// AI Response Parser (to_diff_session)
// ============================================================================

/// AI 응답을 DiffSession으로 변환하는 파서
///
/// AI 응답 포맷 (JSON):
/// ```json
/// {
///   "explanation": "변경 설명",
///   "changes": [
///     {
///       "type": "entity",
///       "entity_id": 123,
///       "entity_name": "Player",
///       "description": "플레이어 위치 변경",
///       "modifications": [
///         {
///           "component": "Transform",
///           "field": "translation.x",
///           "old": 0.0,
///           "new": 10.0
///         }
///       ]
///     },
///     {
///       "type": "code",
///       "file": "scripts/player.lua",
///       "language": "lua",
///       "description": "이동 속도 증가",
///       "diff": "@@ -10,3 +10,3 @@\n speed = 5.0\n-local velocity = 10\n+local velocity = 20"
///     }
///   ]
/// }
/// ```
pub struct AIDiffParser;

impl AIDiffParser {
    /// AI 응답 JSON을 파싱하여 DiffSession 생성
    ///
    /// 이 함수는 AI가 반환한 JSON 응답을 파싱합니다.
    /// 실제 구현에서는 serde_json을 사용하지만, 여기서는 간단한 파서를 제공합니다.
    pub fn parse_response(prompt: &str, response: &str, entity_lookup: &impl Fn(u32) -> Option<Entity>) -> Result<AIDiffSession, DiffParseError> {
        let mut session = AIDiffSession::new(prompt);

        // 간단한 JSON 파싱 (실제로는 serde_json 권장)
        // 여기서는 기본적인 구조만 파싱

        // explanation 추출
        if let Some(exp_start) = response.find("\"explanation\"") {
            if let Some(colon) = response[exp_start..].find(':') {
                let after_colon = &response[exp_start + colon + 1..];
                if let Some(quote_start) = after_colon.find('"') {
                    let rest = &after_colon[quote_start + 1..];
                    if let Some(quote_end) = rest.find('"') {
                        session.explanation = rest[..quote_end].to_string();
                    }
                }
            }
        }

        // changes 배열 파싱은 복잡하므로 여기서는 스킵
        // 실제 구현에서는 serde_json::from_str 사용 권장

        Ok(session)
    }

    /// 엔티티 변경 생성 헬퍼
    pub fn create_entity_change(
        entity: Entity,
        entity_name: &str,
        description: &str,
        modifications: Vec<EntityModification>,
    ) -> DiffChange {
        DiffChange::new(
            DiffChangeType::Entity {
                entity,
                entity_name: entity_name.to_string(),
                modifications,
            },
            description,
        )
    }

    /// 코드 변경 생성 헬퍼
    pub fn create_code_change(
        file_path: impl Into<PathBuf>,
        language: &str,
        description: &str,
        old_content: &str,
        new_content: &str,
    ) -> DiffChange {
        let hunks = Self::compute_diff_hunks(old_content, new_content);

        DiffChange::new(
            DiffChangeType::Code {
                file_path: file_path.into(),
                language: language.to_string(),
                hunks,
            },
            description,
        )
    }

    /// 두 텍스트의 diff hunk 계산
    fn compute_diff_hunks(old: &str, new: &str) -> Vec<DiffHunk> {
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        // 간단한 라인별 비교 (실제로는 Myers diff 알고리즘 권장)
        let mut lines = Vec::new();
        let mut old_idx = 0;
        let mut new_idx = 0;

        while old_idx < old_lines.len() || new_idx < new_lines.len() {
            if old_idx < old_lines.len() && new_idx < new_lines.len() {
                if old_lines[old_idx] == new_lines[new_idx] {
                    // Context
                    lines.push(DiffLine {
                        line_type: DiffLineType::Context,
                        content: old_lines[old_idx].to_string(),
                        old_line_number: Some(old_idx + 1),
                        new_line_number: Some(new_idx + 1),
                    });
                    old_idx += 1;
                    new_idx += 1;
                } else {
                    // 변경됨 - 삭제 후 추가로 표시
                    lines.push(DiffLine {
                        line_type: DiffLineType::Deletion,
                        content: old_lines[old_idx].to_string(),
                        old_line_number: Some(old_idx + 1),
                        new_line_number: None,
                    });
                    lines.push(DiffLine {
                        line_type: DiffLineType::Addition,
                        content: new_lines[new_idx].to_string(),
                        old_line_number: None,
                        new_line_number: Some(new_idx + 1),
                    });
                    old_idx += 1;
                    new_idx += 1;
                }
            } else if old_idx < old_lines.len() {
                // 삭제
                lines.push(DiffLine {
                    line_type: DiffLineType::Deletion,
                    content: old_lines[old_idx].to_string(),
                    old_line_number: Some(old_idx + 1),
                    new_line_number: None,
                });
                old_idx += 1;
            } else {
                // 추가
                lines.push(DiffLine {
                    line_type: DiffLineType::Addition,
                    content: new_lines[new_idx].to_string(),
                    old_line_number: None,
                    new_line_number: Some(new_idx + 1),
                });
                new_idx += 1;
            }
        }

        if lines.is_empty() {
            return Vec::new();
        }

        vec![DiffHunk {
            old_start: 1,
            old_count: old_lines.len(),
            new_start: 1,
            new_count: new_lines.len(),
            lines,
        }]
    }

    /// Transform 수정 생성 헬퍼
    pub fn create_transform_modification(
        field: &str,
        old_value: DiffValue,
        new_value: DiffValue,
    ) -> EntityModification {
        EntityModification {
            component_type: "Transform".to_string(),
            field_path: field.to_string(),
            old_value,
            new_value,
        }
    }
}

/// Diff 파싱 에러
#[derive(Debug, Clone)]
pub enum DiffParseError {
    /// JSON 파싱 실패
    InvalidJson(String),
    /// 필수 필드 누락
    MissingField(String),
    /// 잘못된 변경 타입
    InvalidChangeType(String),
    /// 엔티티를 찾을 수 없음
    EntityNotFound(u32),
}

impl std::fmt::Display for DiffParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "Invalid JSON: {}", msg),
            Self::MissingField(field) => write!(f, "Missing field: {}", field),
            Self::InvalidChangeType(t) => write!(f, "Invalid change type: {}", t),
            Self::EntityNotFound(id) => write!(f, "Entity not found: {}", id),
        }
    }
}

impl std::error::Error for DiffParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_session_lifecycle() {
        let mut session = AIDiffSession::new("Test prompt");

        session.add_change(DiffChange::new(
            DiffChangeType::Entity {
                entity: Entity::from_raw(1),
                entity_name: "Player".to_string(),
                modifications: vec![],
            },
            "Move player",
        ));

        session.add_change(DiffChange::new(
            DiffChangeType::Entity {
                entity: Entity::from_raw(2),
                entity_name: "Enemy".to_string(),
                modifications: vec![],
            },
            "Move enemy",
        ));

        assert_eq!(session.status, DiffSessionStatus::Pending);
        assert_eq!(session.pending_count(), 2);

        session.changes[0].set_accepted(true);
        session.update_status();
        assert_eq!(session.status, DiffSessionStatus::Partial);

        session.accept_all();
        assert_eq!(session.status, DiffSessionStatus::Accepted);
    }

    #[test]
    fn test_diff_value_display() {
        assert_eq!(format!("{}", DiffValue::Int(42)), "42");
        assert_eq!(format!("{}", DiffValue::Float(3.14159)), "3.142");
        assert_eq!(
            format!("{}", DiffValue::Vec3 { x: 1.0, y: 2.0, z: 3.0 }),
            "(1.00, 2.00, 3.00)"
        );
    }

    #[test]
    fn test_diff_hunk_new_file() {
        let content = "line1\nline2\nline3";
        let hunk = DiffHunk::new_file(content);

        assert_eq!(hunk.old_count, 0);
        assert_eq!(hunk.new_count, 3);
        assert_eq!(hunk.lines.len(), 3);
        assert!(hunk.lines.iter().all(|l| l.line_type == DiffLineType::Addition));
    }

    #[test]
    fn test_diff_parser_compute_hunks() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3\nline4";

        let hunks = AIDiffParser::compute_diff_hunks(old, new);
        assert_eq!(hunks.len(), 1);

        let hunk = &hunks[0];
        assert_eq!(hunk.old_count, 3);
        assert_eq!(hunk.new_count, 4);

        // line1 = context, line2 = del/add, line3 = context, line4 = add
        let additions = hunk.lines.iter().filter(|l| l.line_type == DiffLineType::Addition).count();
        let deletions = hunk.lines.iter().filter(|l| l.line_type == DiffLineType::Deletion).count();

        assert!(additions >= 2); // modified + line4
        assert!(deletions >= 1); // line2
    }

    #[test]
    fn test_create_entity_change() {
        let change = AIDiffParser::create_entity_change(
            Entity::from_raw(1),
            "Player",
            "Move player",
            vec![AIDiffParser::create_transform_modification(
                "translation.x",
                DiffValue::Float(0.0),
                DiffValue::Float(10.0),
            )],
        );

        assert_eq!(change.description, "Move player");
        if let DiffChangeType::Entity { entity_name, modifications, .. } = &change.change_type {
            assert_eq!(entity_name, "Player");
            assert_eq!(modifications.len(), 1);
        } else {
            panic!("Expected Entity change type");
        }
    }
}
