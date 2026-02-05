//! 도킹 레이아웃 직렬화
//!
//! 언리얼 FTabManager::FLayout 참고
//! - ToJson/NewFromJson으로 JSON 직렬화
//! - Type, SizeCoefficient, Orientation, Tabs, Nodes 구조

use super::{NodeId, TabId, SplitDirection, DockTree};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// 레이아웃 버전 (호환성 체크용)
pub const LAYOUT_VERSION: u32 = 3;

/// 도킹 레이아웃 (언리얼 FTabManager::FLayout)
///
/// 전체 도킹 상태를 저장하는 최상위 구조
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockLayout {
    /// 레이아웃 버전
    pub version: u32,
    /// 레이아웃 이름 (식별용)
    pub name: String,
    /// 루트 노드
    pub root: Option<LayoutNode>,
    /// 탭 정보 (ID → 탭 이름 매핑)
    pub tab_names: HashMap<u64, String>,
}

impl DockLayout {
    /// 새 레이아웃 생성
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: LAYOUT_VERSION,
            name: name.into(),
            root: None,
            tab_names: HashMap::new(),
        }
    }

    /// JSON으로 직렬화 (언리얼 ToJson)
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// JSON에서 역직렬화 (언리얼 NewFromJson)
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 버전 호환성 체크
    pub fn is_compatible(&self) -> bool {
        self.version == LAYOUT_VERSION
    }
}

impl Default for DockLayout {
    fn default() -> Self {
        Self::new("Default")
    }
}

/// 레이아웃 노드 (언리얼 FLayoutNode)
///
/// 재귀적 트리 구조로 도킹 상태 표현
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LayoutNode {
    /// 탭 스택 (언리얼 ELayoutNodeType::Stack)
    #[serde(rename = "Stack")]
    Stack {
        /// 노드 ID (복원 시 매핑용)
        node_id: u64,
        /// 포함된 탭들
        tabs: Vec<TabLayoutInfo>,
        /// 활성 탭 인덱스
        active_tab: usize,
        /// 크기 계수 (언리얼 SizeCoefficient)
        size_coefficient: f32,
        /// 탭 바 숨김 (UE HideTabWell)
        #[serde(default)]
        hide_tab_well: bool,
    },
    /// 분할자 (언리얼 ELayoutNodeType::Splitter)
    #[serde(rename = "Splitter")]
    Splitter {
        /// 노드 ID
        node_id: u64,
        /// 분할 방향 (언리얼 Orientation)
        orientation: SplitDirection,
        /// 자식 노드들
        nodes: Vec<LayoutNode>,
        /// 각 자식의 크기 계수
        coefficients: Vec<f32>,
    },
}

impl LayoutNode {
    /// 노드 ID 가져오기
    pub fn node_id(&self) -> u64 {
        match self {
            Self::Stack { node_id, .. } => *node_id,
            Self::Splitter { node_id, .. } => *node_id,
        }
    }

    /// 스택 노드 생성
    pub fn new_stack(node_id: NodeId, tabs: Vec<TabLayoutInfo>, active_tab: usize, size_coefficient: f32) -> Self {
        Self::Stack {
            node_id: node_id.0,
            tabs,
            active_tab,
            size_coefficient,
            hide_tab_well: false,
        }
    }

    /// 분할자 노드 생성
    pub fn new_splitter(
        node_id: NodeId,
        orientation: SplitDirection,
        nodes: Vec<LayoutNode>,
        coefficients: Vec<f32>,
    ) -> Self {
        Self::Splitter {
            node_id: node_id.0,
            orientation,
            nodes,
            coefficients,
        }
    }
}

/// 탭 레이아웃 정보 (언리얼 FTab)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabLayoutInfo {
    /// 탭 ID
    pub tab_id: u64,
    /// 탭 이름 (식별용, 복원 시 매칭)
    pub tab_name: String,
    /// 탭 상태
    pub state: TabState,
}

impl TabLayoutInfo {
    pub fn new(tab_id: TabId, tab_name: impl Into<String>) -> Self {
        Self {
            tab_id: tab_id.0,
            tab_name: tab_name.into(),
            state: TabState::Open,
        }
    }

    pub fn with_state(mut self, state: TabState) -> Self {
        self.state = state;
        self
    }
}

/// 탭 상태 (언리얼 ETabState)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabState {
    /// 열려있음
    #[serde(rename = "Open")]
    Open,
    /// 닫혀있음 (복원 가능)
    #[serde(rename = "Closed")]
    Closed,
    /// 사이드바로 최소화
    #[serde(rename = "Sidebar")]
    Sidebar,
}

impl Default for TabState {
    fn default() -> Self {
        Self::Open
    }
}

/// 플로팅 윈도우 레이아웃 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingWindowLayout {
    /// DockTree 레이아웃 (분할 구조 포함)
    #[serde(default)]
    pub dock_tree: Option<DockTree>,
    /// 탭 목록 (v1 호환용, dock_tree 없을 때 사용)
    #[serde(default)]
    pub tabs: Vec<TabLayoutInfo>,
    /// 활성 탭 인덱스 (v1 호환용)
    #[serde(default)]
    pub active_tab: usize,
    /// 윈도우 위치 [x, y] (스크린 좌표)
    pub position: [f32; 2],
    /// 윈도우 크기 [width, height]
    pub size: [f32; 2],
}

/// 에디터 전체 레이아웃 (모든 MajorTab 포함)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLayout {
    /// 레이아웃 버전
    pub version: u32,
    /// 레이아웃 이름
    pub name: String,
    /// 각 MajorTab의 레이아웃
    pub major_tabs: Vec<MajorTabLayout>,
    /// 활성 MajorTab 인덱스
    pub active_major: usize,
    /// 플로팅 윈도우 레이아웃
    #[serde(default)]
    pub floating_windows: Vec<FloatingWindowLayout>,
}

impl EditorLayout {
    /// JSON으로 직렬화
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// JSON에서 역직렬화
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 버전 호환성 체크
    pub fn is_compatible(&self) -> bool {
        self.version == LAYOUT_VERSION
    }
}

/// 개별 MajorTab 레이아웃
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MajorTabLayout {
    /// MajorTab 제목
    pub title: String,
    /// 아이콘
    pub icon: Option<String>,
    /// 닫기 가능 여부
    pub closable: bool,
    /// 내부 DockTree 레이아웃
    pub dock_layout: DockLayout,
    /// 왼쪽 사이드바 탭들
    #[serde(default)]
    pub left_sidebar_tabs: Vec<SidebarTabLayoutInfo>,
    /// 오른쪽 사이드바 탭들
    #[serde(default)]
    pub right_sidebar_tabs: Vec<SidebarTabLayoutInfo>,
}

/// 사이드바 탭 직렬화 정보
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarTabLayoutInfo {
    /// 탭 타입명 (스포너 복원용)
    pub tab_type_name: String,
    /// 표시 이름
    pub display_name: String,
    /// 아이콘
    pub icon: Option<String>,
}

/// 레이아웃 프리셋 (저장된 레이아웃 설정)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPreset {
    /// 프리셋 이름
    pub name: String,
    /// 설명
    pub description: String,
    /// 빌트인 여부 (빌트인은 삭제 불가)
    pub is_builtin: bool,
    /// 직렬화된 레이아웃 JSON
    pub layout_json: String,
}

/// 레이아웃 프리셋 레지스트리
#[derive(Debug, Clone, Default)]
pub struct LayoutPresetRegistry {
    presets: Vec<LayoutPreset>,
}

impl LayoutPresetRegistry {
    pub fn new() -> Self {
        Self { presets: Vec::new() }
    }

    /// 빌트인 프리셋 등록
    pub fn register_builtin(&mut self, name: impl Into<String>, description: impl Into<String>, layout_json: String) {
        self.presets.push(LayoutPreset {
            name: name.into(),
            description: description.into(),
            is_builtin: true,
            layout_json,
        });
    }

    /// 사용자 프리셋 등록
    pub fn register_user(&mut self, name: impl Into<String>, description: impl Into<String>, layout_json: String) {
        let name = name.into();
        // 같은 이름 있으면 덮어쓰기
        self.presets.retain(|p| p.name != name || p.is_builtin);
        self.presets.push(LayoutPreset {
            name,
            description: description.into(),
            is_builtin: false,
            layout_json,
        });
    }

    /// 프리셋 조회
    pub fn get(&self, name: &str) -> Option<&LayoutPreset> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// 모든 프리셋 목록
    pub fn list(&self) -> &[LayoutPreset] {
        &self.presets
    }

    /// 사용자 프리셋 삭제 (빌트인은 삭제 불가)
    pub fn remove_user(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| p.name != name || p.is_builtin);
        self.presets.len() < before
    }

    /// 전체 프리셋을 JSON으로 직렬화
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.presets)
    }

    /// JSON에서 사용자 프리셋 로드 (빌트인은 유지, 사용자 프리셋만 추가/덮어쓰기)
    pub fn from_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let loaded: Vec<LayoutPreset> = serde_json::from_str(json)?;
        for preset in loaded {
            if !preset.is_builtin {
                self.register_user(preset.name, preset.description, preset.layout_json);
            }
        }
        Ok(())
    }

    /// 파일에 프리셋 저장
    pub fn save_to_file(&self, path: impl AsRef<std::path::Path>) -> Result<(), std::io::Error> {
        let json = self.to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, json)
    }

    /// 파일에서 프리셋 로드
    pub fn load_from_file(&mut self, path: impl AsRef<std::path::Path>) -> Result<(), Box<dyn std::error::Error>> {
        let json = std::fs::read_to_string(path)?;
        self.from_json(&json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_serialization() {
        let mut layout = DockLayout::new("TestLayout");

        // 스택 노드 추가
        let tab1 = TabLayoutInfo::new(TabId(1), "Viewport");
        let tab2 = TabLayoutInfo::new(TabId(2), "Hierarchy");

        let stack = LayoutNode::new_stack(
            NodeId::new(100),
            vec![tab1, tab2],
            0,
            1.0,
        );

        layout.root = Some(stack);
        layout.tab_names.insert(1, "Viewport".to_string());
        layout.tab_names.insert(2, "Hierarchy".to_string());

        // JSON 직렬화
        let json = layout.to_json().expect("Serialization failed");
        println!("JSON:\n{}", json);

        // JSON 역직렬화
        let restored = DockLayout::from_json(&json).expect("Deserialization failed");

        assert_eq!(restored.version, LAYOUT_VERSION);
        assert_eq!(restored.name, "TestLayout");
        assert!(restored.root.is_some());
    }

    #[test]
    fn test_splitter_layout() {
        let mut layout = DockLayout::new("SplitterTest");

        let left_stack = LayoutNode::new_stack(
            NodeId::new(1),
            vec![TabLayoutInfo::new(TabId(1), "Left")],
            0,
            0.3,
        );

        let right_stack = LayoutNode::new_stack(
            NodeId::new(2),
            vec![TabLayoutInfo::new(TabId(2), "Right")],
            0,
            0.7,
        );

        let splitter = LayoutNode::new_splitter(
            NodeId::new(100),
            SplitDirection::Horizontal,
            vec![left_stack, right_stack],
            vec![0.3, 0.7],
        );

        layout.root = Some(splitter);

        let json = layout.to_json().expect("Serialization failed");
        println!("Splitter JSON:\n{}", json);

        let restored = DockLayout::from_json(&json).expect("Deserialization failed");

        if let Some(LayoutNode::Splitter { orientation, nodes, coefficients, .. }) = restored.root {
            assert_eq!(orientation, SplitDirection::Horizontal);
            assert_eq!(nodes.len(), 2);
            assert_eq!(coefficients.len(), 2);
        } else {
            panic!("Expected Splitter node");
        }
    }
}
