//! Editor Icon Manager
//!
//! PNG 아이콘을 로드하여 ImGui에서 사용할 수 있는 텍스처로 관리

use std::collections::HashMap;
use std::path::Path;
use dear_imgui_rs::TextureId;
use dear_imgui_wgpu::WgpuRenderer;
use image::GenericImageView;

/// 아이콘 이름 상수
pub mod names {
    // 플레이 컨트롤
    pub const PLAY: &str = "play";
    pub const STOP: &str = "stop";
    pub const HOLD: &str = "hold";  // pause

    // 기즈모
    pub const MOVE: &str = "mov3";
    pub const ROTATE: &str = "turn";
    pub const SCALE: &str = "scale";

    // 그리드
    pub const GRID_ON: &str = "grid_toggle_on";
    pub const GRID_OFF: &str = "grid_toggle_off";

    // 씬/게임
    pub const SCENE: &str = "scene";
    pub const GAME: &str = "game";

    // 패널
    pub const HIERARCHY: &str = "hierachy";
    pub const INSPECTOR: &str = "Inspector";
    pub const CONSOLE: &str = "Console";
    pub const FOLDER: &str = "Folder";
    pub const OPEN_FOLDER: &str = "Open Folder";

    // 가시성
    pub const SEE: &str = "see";
    pub const HIDE: &str = "hide";

    // 토글
    pub const CHECK_ON: &str = "check_toggle_on";
    pub const CHECK_OFF: &str = "check_toggle_off";

    // 타이틀바
    pub const TITLEBAR_CLOSE: &str = "titlebar_x";
    pub const TITLEBAR_MAXIMIZE: &str = "titlebar_sizeup";
    pub const TITLEBAR_RESTORE: &str = "titlebar_sizedown";
    pub const TITLEBAR_MINIMIZE: &str = "titlebar_under";
}

/// 로드된 아이콘 정보
pub struct IconInfo {
    pub texture_id: TextureId,
    pub width: u32,
    pub height: u32,
    // 텍스처는 ImGui에서 참조하므로 유지해야 함
    #[allow(dead_code)]
    texture: wgpu::Texture,
    #[allow(dead_code)]
    view: wgpu::TextureView,
}

/// 아이콘 매니저
pub struct IconManager {
    icons: HashMap<String, IconInfo>,
    /// 아이콘 기본 디렉토리
    icons_dir: String,
}

impl IconManager {
    pub fn new(icons_dir: &str) -> Self {
        Self {
            icons: HashMap::new(),
            icons_dir: icons_dir.to_string(),
        }
    }

    /// 모든 에디터 아이콘 로드
    pub fn load_all(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut WgpuRenderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let icons_dir = self.icons_dir.clone();
        let icons_path = Path::new(&icons_dir);

        if !icons_path.exists() {
            log::warn!("[IconManager] Icons directory not found: {}", icons_dir);
            return Ok(());
        }

        // 메인 아이콘들 로드
        self.load_icons_from_dir(icons_path, "", device, queue, renderer)?;

        // 타이틀바 아이콘들 로드
        let titlebar_path = icons_path.join("titlebar");
        if titlebar_path.exists() {
            self.load_icons_from_dir(&titlebar_path, "titlebar_", device, queue, renderer)?;
        }

        log::info!("[IconManager] Loaded {} icons", self.icons.len());
        Ok(())
    }

    /// 디렉토리에서 PNG 아이콘들 로드
    fn load_icons_from_dir(
        &mut self,
        dir: &Path,
        prefix: &str,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut WgpuRenderer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.to_lowercase() != "png" {
                continue;
            }

            // 파일명에서 아이콘 이름 추출
            let filename = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");

            // "symbol_" 접두사 제거, "_" 로 시작하면 제거
            let name = filename
                .strip_prefix("symbol_").unwrap_or(filename)
                .strip_prefix("_").unwrap_or(filename);

            let full_name = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}{}", prefix, name.strip_prefix("_").unwrap_or(name).to_lowercase())
            };

            match self.load_icon(&path, device, queue, renderer) {
                Ok(info) => {
                    log::debug!("[IconManager] Loaded icon: {} ({}x{})", full_name, info.width, info.height);
                    self.icons.insert(full_name, info);
                }
                Err(e) => {
                    log::warn!("[IconManager] Failed to load icon {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// 단일 PNG 파일 로드
    fn load_icon(
        &self,
        path: &Path,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut WgpuRenderer,
    ) -> Result<IconInfo, Box<dyn std::error::Error>> {
        // image 크레이트로 PNG 로드
        let img = image::open(path)?;
        let (width, height) = img.dimensions();

        // RGBA8로 변환
        let rgba_data = img.to_rgba8();

        // wgpu 텍스처 생성
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Icon: {}", path.display())),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // ImGui 렌더러에 텍스처 등록
        let texture_id = renderer.register_external_texture(&texture, &view);

        Ok(IconInfo {
            texture_id: TextureId::from(texture_id),
            width,
            height,
            texture,
            view,
        })
    }

    /// 아이콘 가져오기
    pub fn get(&self, name: &str) -> Option<&IconInfo> {
        self.icons.get(name)
    }

    /// 아이콘 TextureId 가져오기
    pub fn get_texture_id(&self, name: &str) -> Option<TextureId> {
        self.icons.get(name).map(|info| info.texture_id)
    }

    /// 아이콘이 있는지 확인
    pub fn has(&self, name: &str) -> bool {
        self.icons.contains_key(name)
    }

    /// 로드된 아이콘 개수
    pub fn count(&self) -> usize {
        self.icons.len()
    }

    /// 모든 아이콘 이름 반환
    pub fn list(&self) -> Vec<&String> {
        self.icons.keys().collect()
    }
}

impl Default for IconManager {
    fn default() -> Self {
        Self::new("engine/icons")
    }
}
