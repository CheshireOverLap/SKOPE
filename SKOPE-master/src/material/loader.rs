//! SKOPE Material Loader
//!
//! RON 파일에서 머티리얼 정의를 로드

use std::fs;
use std::path::{Path, PathBuf};

use super::material_def::MaterialDef;
use super::registry::MaterialRegistry;

/// 머티리얼 로더 에러
#[derive(Debug)]
pub enum MaterialLoadError {
    IoError(PathBuf, String),
    ParseError(PathBuf, String),
    DirectoryNotFound(PathBuf),
}

impl std::fmt::Display for MaterialLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IoError(path, e) => write!(f, "Failed to read {:?}: {}", path, e),
            Self::ParseError(path, e) => write!(f, "Failed to parse {:?}: {}", path, e),
            Self::DirectoryNotFound(path) => write!(f, "Directory not found: {:?}", path),
        }
    }
}

impl std::error::Error for MaterialLoadError {}

/// 머티리얼 로더
pub struct MaterialLoader {
    base_path: PathBuf,
}

impl MaterialLoader {
    /// 새 로더 생성
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// 기본 경로 반환
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// 단일 .mat.ron 파일 로드
    pub fn load_file(&self, path: &Path) -> Result<MaterialDef, MaterialLoadError> {
        let content = fs::read_to_string(path)
            .map_err(|e| MaterialLoadError::IoError(path.to_path_buf(), e.to_string()))?;

        let mut def: MaterialDef = ron::from_str(&content)
            .map_err(|e| MaterialLoadError::ParseError(path.to_path_buf(), e.to_string()))?;

        // 텍스처 경로를 RON 파일 기준 상대 경로에서 절대 경로로 변환
        let parent = path.parent().unwrap_or(Path::new("."));
        self.resolve_texture_paths(&mut def, parent);

        Ok(def)
    }

    /// 기본 디렉토리에서 이름으로 로드
    pub fn load(&self, name: &str) -> Result<MaterialDef, MaterialLoadError> {
        let path = self.base_path.join(format!("{}.mat.ron", name));
        self.load_file(&path)
    }

    /// 디렉토리 내 모든 .mat.ron 파일 로드
    pub fn load_directory(&self, registry: &mut MaterialRegistry) -> Result<usize, MaterialLoadError> {
        if !self.base_path.exists() {
            log::warn!(
                "[MaterialLoader] Directory not found: {}",
                self.base_path.display()
            );
            return Ok(0);
        }

        let mut count = 0;

        let entries = fs::read_dir(&self.base_path)
            .map_err(|e| MaterialLoadError::IoError(self.base_path.clone(), e.to_string()))?;

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();

            // .mat.ron 파일만 처리
            if !path.is_file() {
                continue;
            }

            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !filename.ends_with(".mat.ron") {
                continue;
            }

            match self.load_file(&path) {
                Ok(def) => {
                    log::info!("[MaterialLoader] Loaded: {}", def.name);
                    registry.register(def, Some(path));
                    count += 1;
                }
                Err(e) => {
                    log::warn!("[MaterialLoader] Failed to load {:?}: {}", path, e);
                }
            }
        }

        Ok(count)
    }

    /// 텍스처 경로 해석 (RON 파일 기준 상대 경로 -> 절대 경로)
    fn resolve_texture_paths(&self, def: &mut MaterialDef, ron_dir: &Path) {
        if let Some(ref mut path) = def.textures.albedo {
            *path = ron_dir.join(&path);
        }
        if let Some(ref mut path) = def.textures.normal {
            *path = ron_dir.join(&path);
        }
        if let Some(ref mut path) = def.textures.metallic_roughness {
            *path = ron_dir.join(&path);
        }
        if let Some(ref mut path) = def.textures.emissive {
            *path = ron_dir.join(&path);
        }
    }

    /// MaterialDef를 RON 파일로 저장
    pub fn save_file(def: &MaterialDef, path: &Path) -> Result<(), MaterialLoadError> {
        let content = ron::ser::to_string_pretty(def, ron::ser::PrettyConfig::default())
            .map_err(|e| MaterialLoadError::ParseError(path.to_path_buf(), e.to_string()))?;

        fs::write(path, content)
            .map_err(|e| MaterialLoadError::IoError(path.to_path_buf(), e.to_string()))?;

        log::info!("[MaterialLoader] Saved: {} to {:?}", def.name, path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_material_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.mat.ron");

        // 테스트용 RON 파일 생성
        let mut file = fs::File::create(&file_path).unwrap();
        writeln!(
            file,
            r#"(
    name: "TestMaterial",
    base_color: (1.0, 0.5, 0.0, 1.0),
    metallic: 0.5,
    roughness: 0.3,
)"#
        )
        .unwrap();

        let loader = MaterialLoader::new(dir.path());
        let mat = loader.load_file(&file_path).unwrap();

        assert_eq!(mat.name, "TestMaterial");
        assert_eq!(mat.base_color[0], 1.0);
        assert_eq!(mat.metallic, 0.5);
        assert_eq!(mat.roughness, 0.3);
    }
}
