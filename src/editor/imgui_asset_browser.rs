//! ImGui Asset Browser Panel
//!
//! 에셋 파일을 탐색하고 뷰포트로 드래그 앤 드롭

use dear_imgui_rs::{Ui, DragDropFlags};
use std::path::PathBuf;

/// Asset 타입
#[derive(Debug, Clone, PartialEq)]
pub enum AssetType {
    Material,
    Mesh,
    Texture,
    Prefab,
    Scene,
    Script,
    Unknown,
}

impl AssetType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "mat" | "ron" => Self::Material,
            "glb" | "gltf" | "fbx" | "obj" => Self::Mesh,
            "png" | "jpg" | "jpeg" | "ktx2" | "dds" => Self::Texture,
            "prefab" => Self::Prefab,
            "skope" => Self::Scene,
            "lua" => Self::Script,
            _ => Self::Unknown,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Material => "[MAT]",
            Self::Mesh => "[MSH]",
            Self::Texture => "[TEX]",
            Self::Prefab => "[PRF]",
            Self::Scene => "[SCN]",
            Self::Script => "[LUA]",
            Self::Unknown => "[???]",
        }
    }
}

/// Asset 엔트리
#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub name: String,
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub is_directory: bool,
}

/// Asset Browser 상태
pub struct ImGuiAssetBrowserState {
    /// 현재 경로
    pub current_path: PathBuf,
    /// 캐시된 에셋 목록
    pub cached_assets: Vec<AssetEntry>,
    /// 선택된 에셋
    pub selected_asset: Option<usize>,
    /// 캐시 갱신 필요 여부
    pub needs_refresh: bool,
}

impl Default for ImGuiAssetBrowserState {
    fn default() -> Self {
        Self::new()
    }
}

impl ImGuiAssetBrowserState {
    pub fn new() -> Self {
        Self {
            current_path: PathBuf::from("assets"),
            cached_assets: Vec::new(),
            selected_asset: None,
            needs_refresh: true,
        }
    }

    /// 에셋 목록 갱신
    pub fn refresh(&mut self) {
        self.cached_assets.clear();

        // 실제 파일 시스템 스캔 (현재는 하드코딩된 에셋)
        // TODO: 실제 파일 시스템에서 에셋 스캔
        self.cached_assets = vec![
            AssetEntry {
                name: "Materials".to_string(),
                path: self.current_path.join("materials"),
                asset_type: AssetType::Unknown,
                is_directory: true,
            },
            AssetEntry {
                name: "Meshes".to_string(),
                path: self.current_path.join("meshes"),
                asset_type: AssetType::Unknown,
                is_directory: true,
            },
            AssetEntry {
                name: "Textures".to_string(),
                path: self.current_path.join("textures"),
                asset_type: AssetType::Unknown,
                is_directory: true,
            },
            AssetEntry {
                name: "Scenes".to_string(),
                path: self.current_path.join("levels"),
                asset_type: AssetType::Unknown,
                is_directory: true,
            },
            // 예시 에셋들
            AssetEntry {
                name: "M_Default.mat".to_string(),
                path: self.current_path.join("materials/M_Default.mat"),
                asset_type: AssetType::Material,
                is_directory: false,
            },
            AssetEntry {
                name: "SM_Cube.glb".to_string(),
                path: self.current_path.join("meshes/SM_Cube.glb"),
                asset_type: AssetType::Mesh,
                is_directory: false,
            },
            AssetEntry {
                name: "SM_Sphere.glb".to_string(),
                path: self.current_path.join("meshes/SM_Sphere.glb"),
                asset_type: AssetType::Mesh,
                is_directory: false,
            },
        ];

        self.needs_refresh = false;
    }

    /// 디렉토리로 이동
    pub fn navigate_to(&mut self, path: PathBuf) {
        self.current_path = path;
        self.needs_refresh = true;
        self.selected_asset = None;
    }

    /// 상위 디렉토리로 이동
    pub fn navigate_up(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            self.current_path = parent.to_path_buf();
            self.needs_refresh = true;
            self.selected_asset = None;
        }
    }
}

/// Asset Browser 액션
#[derive(Debug, Clone, Default)]
pub enum AssetBrowserAction {
    /// 아무 액션 없음
    #[default]
    None,
    /// 파일 열기 (더블클릭)
    OpenFile(PathBuf),
    /// UI 레이아웃 생성
    CreateUiLayout,
    /// 폴더 생성
    CreateFolder,
    /// 디렉토리 이동
    NavigateTo(PathBuf),
    /// 씬 로드
    LoadScene(PathBuf),
    /// 에셋을 씬에 드롭 (뷰포트 위치에 스폰)
    SpawnAsset {
        asset_path: PathBuf,
        asset_type: AssetType,
    },
    /// 에셋을 엔티티에 적용 (Material 드롭 등)
    ApplyToEntity {
        asset_path: PathBuf,
        asset_type: AssetType,
    },
}

/// Asset Browser 패널 렌더링
pub fn render_asset_browser_panel(
    ui: &Ui,
    state: &mut ImGuiAssetBrowserState,
) -> AssetBrowserAction {
    let mut action = AssetBrowserAction::None;

    // 필요시 캐시 갱신
    if state.needs_refresh {
        state.refresh();
    }

    ui.window("Assets")
        .build(|| {
            // 경로 네비게이션
            ui.text("Content Browser");
            ui.same_line();
            if ui.small_button("<") {
                state.navigate_up();
            }
            ui.same_line();
            ui.text(state.current_path.to_string_lossy().to_string());
            ui.separator();

            // 에셋 그리드/리스트
            let size = ui.content_region_avail();

            // 네비게이션 대기열 (루프 안에서 직접 navigate_to 호출 시 borrow 문제 발생)
            let mut pending_navigation: Option<PathBuf> = None;
            let mut pending_selection: Option<usize> = None;

            ui.child_window("asset_list")
                .size([size[0], size[1]])
                .build(ui, || {
                    for (idx, asset) in state.cached_assets.iter().enumerate() {
                        let is_selected = state.selected_asset == Some(idx);
                        let display_name = if asset.is_directory {
                            format!("[DIR] {}", asset.name)
                        } else {
                            format!("{} {}", asset.asset_type.icon(), asset.name)
                        };

                        if ui.selectable_config(&display_name)
                            .selected(is_selected)
                            .build()
                        {
                            if asset.is_directory {
                                // 디렉토리 클릭 시 이동 (나중에 처리)
                                pending_navigation = Some(asset.path.clone());
                            } else {
                                pending_selection = Some(idx);
                            }
                        }

                        // 더블클릭으로 열기
                        if ui.is_item_hovered() && ui.is_mouse_double_clicked(dear_imgui_rs::MouseButton::Left) {
                            if !asset.is_directory {
                                // 씬 파일은 LoadScene으로 처리
                                if asset.asset_type == AssetType::Scene {
                                    action = AssetBrowserAction::LoadScene(asset.path.clone());
                                } else {
                                    action = AssetBrowserAction::OpenFile(asset.path.clone());
                                }
                            }
                        }

                        // 드래그 소스 (디렉토리가 아닌 경우만)
                        if !asset.is_directory {
                            if let Some(_drag) = ui.drag_drop_source_config("ASSET_DND")
                                .begin_payload(idx as u64)
                            {
                                // 드래그 중 표시
                                ui.text(&asset.name);
                            }
                        }
                    }

                    // 빈 공간에 드롭 타겟 (파일 가져오기용)
                    if let Some(_target) = ui.drag_drop_target() {
                        // TODO: 외부 파일 드롭 처리
                    }
                });

            // 네비게이션/선택 처리 (루프 완료 후)
            if let Some(path) = pending_navigation {
                state.navigate_to(path);
            }
            if let Some(idx) = pending_selection {
                state.selected_asset = Some(idx);
            }
        });

    action
}

/// 뷰포트의 드롭 타겟 체크
/// 뷰포트에서 호출하여 에셋 드롭을 받아들임
pub fn check_viewport_drop_target(
    ui: &Ui,
    state: &ImGuiAssetBrowserState,
) -> AssetBrowserAction {
    let mut action = AssetBrowserAction::None;

    if let Some(target) = ui.drag_drop_target() {
        if let Some(Ok(payload)) = target.accept_payload::<u64, _>("ASSET_DND", DragDropFlags::NONE) {
            if payload.delivery {
                let asset_idx = payload.data as usize;
                if let Some(asset) = state.cached_assets.get(asset_idx) {
                    action = AssetBrowserAction::SpawnAsset {
                        asset_path: asset.path.clone(),
                        asset_type: asset.asset_type.clone(),
                    };
                }
            }
        }
    }

    action
}
