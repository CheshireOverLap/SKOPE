//! Hot Reload Functions for State
//!
//! Shader and material hot reload initialization and management (debug mode only)

use super::State;
use crate::paths;

impl State {
    /// 셰이더 핫 리로드 초기화 (디버그 모드 전용)
    #[cfg(debug_assertions)]
    pub fn init_shader_hot_reload() -> Option<crate::shaders::ShaderHotReload> {
        use crate::shaders::ShaderHotReload;

        // 셰이더 디렉토리 경로
        let shader_path = std::path::Path::new("src/shaders");

        if !shader_path.exists() {
            log::warn!("[ShaderHotReload] Shader directory not found: {:?}", shader_path);
            return None;
        }

        let mut hot_reload = ShaderHotReload::new(shader_path);

        // 주요 셰이더 등록 (src/shaders/ 내의 파일만)
        hot_reload.track("material_eval", "material_eval.wgsl");
        hot_reload.track("visibility", "visibility.wgsl");
        hot_reload.track("debug_draw", "debug_draw.wgsl");
        // Note: grid.wgsl은 src/editor/shaders/에 있어서 별도 관리

        // 파일 감시 시작
        if let Err(e) = hot_reload.start_watching() {
            log::error!("[ShaderHotReload] Failed to start watching: {}", e);
            return None;
        }

        log::info!("[ShaderHotReload] Initialized with {} shaders", 3);
        Some(hot_reload)
    }

    /// 머티리얼 핫 리로드 초기화 (디버그 모드 전용)
    #[cfg(debug_assertions)]
    pub fn init_material_hot_reload() -> Option<crate::material::MaterialHotReload> {
        use crate::material::MaterialHotReload;

        let material_path = std::path::Path::new(paths::game::MATERIALS);

        if !material_path.exists() {
            log::warn!("[MaterialHotReload] Material directory not found: {:?}", material_path);
            return None;
        }

        let mut hot_reload = MaterialHotReload::new(material_path);

        if let Err(e) = hot_reload.start_watching() {
            log::error!("[MaterialHotReload] Failed to start watching: {}", e);
            return None;
        }

        log::info!("[MaterialHotReload] Initialized, watching: {:?}", material_path);
        Some(hot_reload)
    }

    /// 셰이더 핫 리로드 체크 (매 프레임 호출)
    ///
    /// 변경된 셰이더가 있으면 재컴파일 시도
    #[cfg(debug_assertions)]
    pub fn check_shader_hot_reload(&mut self) -> Vec<String> {
        let hot_reload = match &mut self.shader_hot_reload {
            Some(hr) => hr,
            None => return Vec::new(),
        };

        let changed = hot_reload.check_changes();

        if !changed.is_empty() {
            log::info!("[ShaderHotReload] {} shader(s) changed: {:?}", changed.len(), changed);
        }

        changed
    }

    /// 셰이더 재컴파일 및 파이프라인 재생성
    #[cfg(debug_assertions)]
    pub fn reload_shader(&mut self, name: &str) -> Result<(), String> {
        let hot_reload = match &mut self.shader_hot_reload {
            Some(hr) => hr,
            None => return Err("Hot reload not initialized".into()),
        };

        // 셰이더 소스 가져오기
        let source = match hot_reload.get_source(name) {
            Ok(s) => s,
            Err(e) => {
                log::error!("[ShaderHotReload] Failed to load '{}': {}", name, e);
                return Err(format!("Failed to load: {}", e));
            }
        };

        // 파이프라인 재생성
        match name {
            "material_eval" => {
                log::info!("[ShaderHotReload] Rebuilding material_eval pipeline...");
                if let Err(e) = self.deferred_renderer.material_eval.rebuild_pipeline(&self.device, &source) {
                    log::error!("[ShaderHotReload] material_eval rebuild failed: {}", e);
                    return Err(format!("Rebuild failed: {}", e));
                }
            }
            "visibility" => {
                log::info!("[ShaderHotReload] Rebuilding visibility pipeline...");
                if let Err(e) = self.deferred_renderer.visibility_pipeline.rebuild_pipeline(&self.device, &source) {
                    log::error!("[ShaderHotReload] visibility pipeline rebuild failed: {}", e);
                    return Err(format!("Rebuild failed: {}", e));
                }
            }
            "debug_draw" => {
                log::info!("[ShaderHotReload] Rebuilding debug_draw pipeline...");
                if let Err(e) = self.debug_draw_renderer.rebuild_pipeline(&self.device, &source) {
                    log::error!("[ShaderHotReload] debug_draw rebuild failed: {}", e);
                    return Err(format!("Rebuild failed: {}", e));
                }
            }
            _ => {
                log::warn!("[ShaderHotReload] Unknown shader: {}", name);
            }
        }

        log::info!("[ShaderHotReload] Successfully reloaded '{}'", name);
        Ok(())
    }
}
