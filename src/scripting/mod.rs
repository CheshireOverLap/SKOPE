// SKOPE Scripting System
// Lua + Rust 하이브리드 스크립팅
#![allow(dead_code)]

use mlua::{Lua, Result as LuaResult, Table, Function};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::SystemTime;
use glam::{Vec3, Quat};
use bevy_ecs::prelude::*;

use crate::ecs_components::Transform;
use crate::ecs_resources;

pub mod api;

/// 스크립트 컴포넌트 - 엔티티에 부착
#[derive(Component)]
pub struct LuaScript {
    /// 스크립트 파일 경로
    pub path: PathBuf,
    /// 스크립트 인스턴스 ID (Lua 테이블 레지스트리 키)
    pub instance_id: Option<i64>,
    /// 마지막 수정 시간 (핫 리로드용)
    pub last_modified: Option<SystemTime>,
    /// 활성화 여부
    pub enabled: bool,
}

impl LuaScript {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            instance_id: None,
            last_modified: None,
            enabled: true,
        }
    }
}

/// Lua 스크립팅 엔진
/// Note: Lua is not Send+Sync, so this must be used as NonSend resource
pub struct ScriptEngine {
    /// Lua 상태
    lua: Lua,
    /// 로드된 스크립트들
    loaded_scripts: HashMap<PathBuf, LoadedScript>,
    /// 다음 인스턴스 ID
    next_instance_id: i64,
    /// 스크립트 기본 경로
    base_path: PathBuf,
    /// 핫 리로드 활성화
    hot_reload_enabled: bool,
}

/// 로드된 스크립트 정보
struct LoadedScript {
    /// 스크립트 내용 해시 (변경 감지용)
    content_hash: u64,
    /// 마지막 수정 시간
    last_modified: SystemTime,
}

impl ScriptEngine {
    /// 새 스크립트 엔진 생성
    pub fn new() -> LuaResult<Self> {
        let lua = Lua::new();

        // 기본 라이브러리 로드 (안전한 것들만)
        lua.globals().set("print", lua.create_function(|_, args: mlua::Variadic<String>| {
            let msg = args.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\t");
            println!("[Lua] {}", msg);
            Ok(())
        })?)?;

        // SKOPE 네임스페이스 생성
        let skope = lua.create_table()?;
        lua.globals().set("SKOPE", skope)?;

        Ok(Self {
            lua,
            loaded_scripts: HashMap::new(),
            next_instance_id: 1,
            base_path: PathBuf::from("assets/scripts"),
            hot_reload_enabled: true,
        })
    }

    /// 스크립트 기본 경로 설정
    pub fn set_base_path(&mut self, path: impl Into<PathBuf>) {
        self.base_path = path.into();
    }

    /// API 초기화 (World 접근 필요한 API들)
    pub fn init_api(&self) -> LuaResult<()> {
        api::register_all(&self.lua)?;
        Ok(())
    }

    /// 스크립트 로드
    pub fn load_script(&mut self, path: &Path) -> LuaResult<i64> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        };

        // 파일 읽기
        let content = fs::read_to_string(&full_path)
            .map_err(|e| mlua::Error::external(format!("Failed to read script: {}", e)))?;

        // 스크립트 실행하여 클래스 테이블 얻기
        let chunk = self.lua.load(&content).set_name(full_path.to_string_lossy());
        let script_table: Table = chunk.eval()?;

        // 인스턴스 생성
        let instance_id = self.next_instance_id;
        self.next_instance_id += 1;

        // 레지스트리에 저장
        self.lua.set_named_registry_value(&format!("script_{}", instance_id), script_table)?;

        // 메타데이터 저장
        let metadata = fs::metadata(&full_path).ok();
        self.loaded_scripts.insert(full_path.clone(), LoadedScript {
            content_hash: Self::hash_content(&content),
            last_modified: metadata.and_then(|m| m.modified().ok()).unwrap_or(SystemTime::UNIX_EPOCH),
        });

        println!("[Script] Loaded: {} (instance #{})", full_path.display(), instance_id);

        Ok(instance_id)
    }

    /// 스크립트 핫 리로드 체크
    pub fn check_hot_reload(&mut self) -> Vec<PathBuf> {
        if !self.hot_reload_enabled {
            return Vec::new();
        }

        let mut reloaded = Vec::new();

        for (path, script) in &self.loaded_scripts {
            if let Ok(metadata) = fs::metadata(path) {
                if let Ok(modified) = metadata.modified() {
                    if modified > script.last_modified {
                        reloaded.push(path.clone());
                    }
                }
            }
        }

        reloaded
    }

    /// 스크립트 리로드
    pub fn reload_script(&mut self, path: &Path, instance_id: i64) -> LuaResult<()> {
        let full_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.base_path.join(path)
        };

        let content = fs::read_to_string(&full_path)
            .map_err(|e| mlua::Error::external(format!("Failed to read script: {}", e)))?;

        // 새 테이블로 교체
        let chunk = self.lua.load(&content).set_name(full_path.to_string_lossy());
        let script_table: Table = chunk.eval()?;

        self.lua.set_named_registry_value(&format!("script_{}", instance_id), script_table)?;

        // 메타데이터 업데이트
        if let Some(script) = self.loaded_scripts.get_mut(&full_path) {
            script.content_hash = Self::hash_content(&content);
            script.last_modified = fs::metadata(&full_path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
        }

        println!("[Script] Hot reloaded: {}", full_path.display());

        Ok(())
    }

    /// 스크립트 함수 호출
    pub fn call_method(&self, instance_id: i64, method: &str, args: impl mlua::IntoLuaMulti) -> LuaResult<()> {
        let script_table: Table = self.lua.named_registry_value(&format!("script_{}", instance_id))?;

        if let Ok(func) = script_table.get::<Function>(method) {
            func.call::<()>((script_table.clone(), args))?;
        }

        Ok(())
    }

    /// 스크립트에 값 전달
    pub fn call_method_with_context<R: mlua::FromLuaMulti>(
        &self,
        instance_id: i64,
        method: &str,
        ctx: &ScriptContext,
    ) -> LuaResult<R> {
        let script_table: Table = self.lua.named_registry_value(&format!("script_{}", instance_id))?;

        // 컨텍스트를 Lua 테이블로 변환
        let ctx_table = self.lua.create_table()?;
        ctx_table.set("entity_id", ctx.entity_id)?;
        ctx_table.set("delta_time", ctx.delta_time)?;

        // position
        let pos_table = self.lua.create_table()?;
        pos_table.set("x", ctx.position.x)?;
        pos_table.set("y", ctx.position.y)?;
        pos_table.set("z", ctx.position.z)?;
        ctx_table.set("position", pos_table)?;

        if let Ok(func) = script_table.get::<Function>(method) {
            func.call((script_table.clone(), ctx_table))
        } else {
            // 메서드가 없으면 기본값 반환
            Err(mlua::Error::external(format!("Method '{}' not found", method)))
        }
    }

    /// on_start 호출
    pub fn call_start(&self, instance_id: i64, ctx: &ScriptContext) -> LuaResult<()> {
        self.call_method_with_context::<()>(instance_id, "on_start", ctx).ok();
        Ok(())
    }

    /// on_update 호출
    pub fn call_update(&self, instance_id: i64, ctx: &ScriptContext) -> LuaResult<()> {
        self.call_method_with_context::<()>(instance_id, "on_update", ctx).ok();
        Ok(())
    }

    /// on_destroy 호출
    pub fn call_destroy(&self, instance_id: i64) -> LuaResult<()> {
        self.call_method(instance_id, "on_destroy", ()).ok();
        Ok(())
    }

    /// 글로벌 변수 설정
    pub fn set_global<V: mlua::IntoLua>(&self, name: &str, value: V) -> LuaResult<()> {
        self.lua.globals().set(name, value)
    }

    /// 글로벌 변수 가져오기
    pub fn get_global<V: mlua::FromLua>(&self, name: &str) -> LuaResult<V> {
        self.lua.globals().get(name)
    }

    fn hash_content(content: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }

    /// Input 상태 업데이트 (매 프레임 호출)
    pub fn update_input(&self, mouse_x: f32, mouse_y: f32, delta_x: f32, delta_y: f32) -> LuaResult<()> {
        api::update_input_state(&self.lua, mouse_x, mouse_y, delta_x, delta_y)
    }

    /// 키 상태 업데이트
    pub fn update_key(&self, key: &str, pressed: bool) -> LuaResult<()> {
        api::update_key_state(&self.lua, key, pressed)
    }

    /// Time 상태 업데이트 (매 프레임 호출)
    pub fn update_time(&self, delta: f32, elapsed: f32, frame_count: u64, fps: f32) -> LuaResult<()> {
        api::update_time(&self.lua, delta, elapsed, frame_count, fps)
    }

    /// Lua VM 참조 (내부 사용)
    pub fn lua(&self) -> &Lua {
        &self.lua
    }
}

/// 스크립트 실행 컨텍스트
#[derive(Debug, Clone)]
pub struct ScriptContext {
    /// 현재 엔티티 ID
    pub entity_id: u64,
    /// 델타 타임
    pub delta_time: f32,
    /// 현재 위치
    pub position: Vec3,
    /// 현재 회전
    pub rotation: Quat,
    /// 현재 스케일
    pub scale: Vec3,
}

impl Default for ScriptContext {
    fn default() -> Self {
        Self {
            entity_id: 0,
            delta_time: 0.0,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

/// 스크립트 시스템 - ECS 시스템으로 스크립트 실행
pub fn script_update_system(
    mut script_engine: Option<NonSendMut<ScriptEngine>>,
    mut query: Query<(Entity, &mut LuaScript, Option<&Transform>)>,
    time: Option<Res<ecs_resources::Time>>,
) {
    let Some(ref mut engine) = script_engine else { return };
    let delta_time = time.map(|t| t.delta_seconds).unwrap_or(0.016);

    for (entity, mut script, transform) in query.iter_mut() {
        if !script.enabled {
            continue;
        }

        // 스크립트가 아직 로드되지 않았으면 로드
        if script.instance_id.is_none() {
            match engine.load_script(&script.path) {
                Ok(id) => {
                    script.instance_id = Some(id);

                    // on_start 호출
                    let ctx = create_context(entity, delta_time, transform);
                    let _ = engine.call_start(id, &ctx);
                }
                Err(e) => {
                    eprintln!("[Script] Error loading {:?}: {}", script.path, e);
                    script.enabled = false;
                }
            }
        }

        // on_update 호출
        if let Some(id) = script.instance_id {
            let ctx = create_context(entity, delta_time, transform);
            if let Err(e) = engine.call_update(id, &ctx) {
                eprintln!("[Script] Error in update: {}", e);
            }
        }
    }
}

fn create_context(entity: Entity, delta_time: f32, transform: Option<&Transform>) -> ScriptContext {
    ScriptContext {
        entity_id: entity.to_bits(),
        delta_time,
        position: transform.map(|t| t.translation).unwrap_or(Vec3::ZERO),
        rotation: transform.map(|t| t.rotation).unwrap_or(Quat::IDENTITY),
        scale: transform.map(|t| t.scale).unwrap_or(Vec3::ONE),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_creation() {
        let engine = ScriptEngine::new().unwrap();
        assert!(engine.loaded_scripts.is_empty());
    }

    #[test]
    fn test_lua_print() {
        let engine = ScriptEngine::new().unwrap();
        engine.lua.load("print('Hello from Lua!')").exec().unwrap();
    }
}
