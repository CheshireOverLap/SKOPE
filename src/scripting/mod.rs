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
pub mod sandbox;
pub mod validator;
pub mod error;
pub mod watcher;
pub mod ui_commands;
pub mod ui_api;

// Re-export for convenience
pub use api::EntityTransform;
pub use api::DebugDrawCommand;
pub use api::{SpellCommand, TriggerEvent, TriggerEventType, TriggerDefinition};

// Sandboxing and validation
pub use sandbox::{TrustLevel, create_sandboxed_lua, validate_code};
pub use validator::AiCodeValidator;
pub use error::{ErrorSeverity, LuaErrorInfo, ErrorReporter};

// File watching
#[allow(unused_imports)]
pub use watcher::{ScriptWatcher, WatcherError};

// UI API
pub use ui_commands::{UiCommand, LuaBindingValue, UiEventType, LuaUiEvent, WidgetDefinition};
pub use ui_api::{process_ui_commands, sync_widget_registry, sync_ui_state, dispatch_ui_event, WidgetInfo, UiState};

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
    /// 신뢰 레벨 (샌드박싱 수준)
    trust_level: TrustLevel,
    /// AI 코드 검증기
    validator: AiCodeValidator,
    /// 에러 리포터
    error_reporter: ErrorReporter,
    /// 검증 활성화
    validation_enabled: bool,
    /// 파일 시스템 감시기 (핫 리로드용)
    watcher: Option<watcher::ScriptWatcher>,
}

/// 로드된 스크립트 정보
struct LoadedScript {
    /// 스크립트 내용 해시 (변경 감지용)
    content_hash: u64,
    /// 마지막 수정 시간
    last_modified: SystemTime,
}

impl ScriptEngine {
    /// 새 스크립트 엔진 생성 (기본: GameScript 신뢰 레벨)
    pub fn new() -> LuaResult<Self> {
        Self::with_trust_level(TrustLevel::GameScript)
    }

    /// 샌드박싱된 스크립트 엔진 생성 (AI 생성 코드용)
    pub fn new_sandboxed() -> LuaResult<Self> {
        Self::with_trust_level(TrustLevel::AiGenerated)
    }

    /// 특정 신뢰 레벨로 스크립트 엔진 생성
    pub fn with_trust_level(trust_level: TrustLevel) -> LuaResult<Self> {
        // 신뢰 레벨에 따라 Lua 인스턴스 생성
        let lua = if trust_level == TrustLevel::Engine || trust_level == TrustLevel::GameScript {
            // 신뢰할 수 있는 코드는 일반 Lua 사용
            Lua::new()
        } else {
            // 신뢰할 수 없는 코드는 샌드박싱된 Lua 사용
            create_sandboxed_lua(trust_level)?
        };

        // 기본 라이브러리 로드 (안전한 것들만)
        lua.globals().set("print", lua.create_function(|_, args: mlua::Variadic<String>| {
            let msg = args.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\t");
            log::info!("[Lua] {}", msg);
            Ok(())
        })?)?;

        // SKOPE 네임스페이스 생성
        let skope = lua.create_table()?;
        lua.globals().set("SKOPE", skope)?;

        log::info!("[ScriptEngine] Created with trust level: {:?}", trust_level);

        // 파일 감시기 초기화 (실패해도 폴링으로 폴백)
        let watcher = match watcher::ScriptWatcher::new() {
            Ok(w) => {
                log::info!("[ScriptEngine] File watcher initialized");
                Some(w)
            }
            Err(e) => {
                log::warn!("[ScriptEngine] File watcher failed: {}, using polling fallback", e);
                None
            }
        };

        Ok(Self {
            lua,
            loaded_scripts: HashMap::new(),
            next_instance_id: 1,
            base_path: PathBuf::from("assets/scripts"),
            hot_reload_enabled: true,
            trust_level,
            validator: AiCodeValidator::new(),
            error_reporter: ErrorReporter::new(),
            validation_enabled: trust_level != TrustLevel::Engine,
            watcher,
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

        // 검증 수행 (활성화된 경우)
        if self.validation_enabled {
            // 샌드박스 정적 검증
            if let Err(msg) = validate_code(&content, self.trust_level) {
                let error_info = LuaErrorInfo::validation(
                    full_path.to_str().unwrap_or("unknown"),
                    &msg,
                    None,
                    0.0,
                );
                self.error_reporter.report(error_info);
                return Err(mlua::Error::external(format!("Validation failed: {}", msg)));
            }

            // AI 코드 검증기 (AiGenerated, UserScript 레벨)
            if self.trust_level == TrustLevel::AiGenerated || self.trust_level == TrustLevel::UserScript {
                let result = self.validator.validate(&content);
                if !result.is_valid {
                    for err in &result.errors {
                        let error_info = LuaErrorInfo::validation(
                            full_path.to_str().unwrap_or("unknown"),
                            &err.message,
                            err.line.map(|l| l as u32),
                            0.0,
                        );
                        self.error_reporter.report(error_info);
                    }
                    let first_error = result.errors.first()
                        .map(|e| e.message.clone())
                        .unwrap_or_else(|| "Unknown validation error".to_string());
                    return Err(mlua::Error::external(format!("AI validation failed: {}", first_error)));
                }

                // 경고 로그
                for warning in &result.warnings {
                    log::warn!("[Script] {}: line {:?} - {}",
                        full_path.display(),
                        warning.line,
                        warning.message
                    );
                }

                log::info!("[Script] Validation passed (lines: {}, functions: {}, loops: {})",
                    result.metrics.line_count,
                    result.metrics.function_count,
                    result.metrics.loop_count
                );
            }
        }

        // 스크립트 실행하여 클래스 테이블 얻기
        let chunk = self.lua.load(&content).set_name(full_path.to_string_lossy());
        let script_table: Table = match chunk.eval() {
            Ok(t) => t,
            Err(e) => {
                let error_info = LuaErrorInfo::from_mlua_error(
                    &e,
                    full_path.to_str().unwrap_or("unknown"),
                    0.0,
                );
                self.error_reporter.report(error_info);
                return Err(e);
            }
        };

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

        log::info!("[Script] Loaded: {} (instance #{}, trust: {:?})",
            full_path.display(), instance_id, self.trust_level);

        Ok(instance_id)
    }

    /// 스크립트 핫 리로드 체크 (watcher 사용 시 이벤트 기반, 없으면 폴링)
    pub fn check_hot_reload(&mut self) -> Vec<PathBuf> {
        if !self.hot_reload_enabled {
            return Vec::new();
        }

        // watcher가 있으면 이벤트 기반으로 체크
        if let Some(ref mut watcher) = self.watcher {
            let changes = watcher.poll_changes();
            // 로드된 스크립트 중에서 변경된 것만 필터링
            return changes
                .into_iter()
                .filter(|path| self.loaded_scripts.contains_key(path))
                .collect();
        }

        // 폴링 폴백 (watcher 없을 때)
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

    /// 스크립트 경로 감시 시작 (watcher 사용 시)
    pub fn start_watching(&mut self, path: impl AsRef<Path>) -> Result<(), WatcherError> {
        if let Some(ref mut watcher) = self.watcher {
            watcher.watch(path)?;
        }
        Ok(())
    }

    /// 스크립트 기본 경로 감시 시작
    pub fn start_watching_base_path(&mut self) -> Result<(), WatcherError> {
        let base = self.base_path.clone();
        self.start_watching(&base)
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

        log::info!("[Script] Hot reloaded: {}", full_path.display());

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

    /// on_update 호출 (반환값 파싱하여 ScriptResult로 변환)
    pub fn call_update(&self, instance_id: i64, ctx: &ScriptContext) -> ScriptResult {
        let script_table: Table = match self.lua.named_registry_value(&format!("script_{}", instance_id)) {
            Ok(t) => t,
            Err(_) => return ScriptResult::empty(),
        };

        // 컨텍스트를 Lua 테이블로 변환
        let ctx_table = match self.lua.create_table() {
            Ok(t) => t,
            Err(_) => return ScriptResult::empty(),
        };

        let _ = ctx_table.set("entity_id", ctx.entity_id);
        let _ = ctx_table.set("delta_time", ctx.delta_time);

        // position
        if let Ok(pos_table) = self.lua.create_table() {
            let _ = pos_table.set("x", ctx.position.x);
            let _ = pos_table.set("y", ctx.position.y);
            let _ = pos_table.set("z", ctx.position.z);
            let _ = ctx_table.set("position", pos_table);
        }

        // rotation (쿼터니언)
        if let Ok(rot_table) = self.lua.create_table() {
            let _ = rot_table.set("x", ctx.rotation.x);
            let _ = rot_table.set("y", ctx.rotation.y);
            let _ = rot_table.set("z", ctx.rotation.z);
            let _ = rot_table.set("w", ctx.rotation.w);
            let _ = ctx_table.set("rotation", rot_table);
        }

        // scale
        if let Ok(scale_table) = self.lua.create_table() {
            let _ = scale_table.set("x", ctx.scale.x);
            let _ = scale_table.set("y", ctx.scale.y);
            let _ = scale_table.set("z", ctx.scale.z);
            let _ = ctx_table.set("scale", scale_table);
        }

        // on_update 함수 호출
        let func: Function = match script_table.get("on_update") {
            Ok(f) => f,
            Err(_) => return ScriptResult::empty(),
        };

        let result: Option<Table> = match func.call((script_table.clone(), ctx_table)) {
            Ok(r) => r,
            Err(e) => {
                log::info!("[Script] Error in on_update: {}", e);
                return ScriptResult::empty();
            }
        };

        // 반환값 파싱
        self.parse_script_result(result)
    }

    /// Lua 반환 테이블을 ScriptResult로 파싱
    fn parse_script_result(&self, result: Option<Table>) -> ScriptResult {
        let Some(table) = result else {
            return ScriptResult::empty();
        };

        let mut script_result = ScriptResult::empty();

        // position 파싱
        if let Ok(pos_table) = table.get::<Table>("position") {
            let x: f32 = pos_table.get("x").unwrap_or(0.0);
            let y: f32 = pos_table.get("y").unwrap_or(0.0);
            let z: f32 = pos_table.get("z").unwrap_or(0.0);
            script_result.position = Some(Vec3::new(x, y, z));
        }

        // rotation 파싱 - 두 가지 형식 지원
        // 1. 쿼터니언: { x, y, z, w }
        // 2. 축-각도: { axis = "y", angle = 1.57 }
        if let Ok(rot_table) = table.get::<Table>("rotation") {
            // 먼저 축-각도 형식 체크
            if let Ok(axis) = rot_table.get::<String>("axis") {
                let angle: f32 = rot_table.get("angle").unwrap_or(0.0);
                let axis_vec = match axis.as_str() {
                    "x" => Vec3::X,
                    "y" => Vec3::Y,
                    "z" => Vec3::Z,
                    _ => Vec3::Y,
                };
                script_result.rotation = Some(Quat::from_axis_angle(axis_vec, angle));
            }
            // 쿼터니언 형식
            else if let (Ok(x), Ok(y), Ok(z), Ok(w)) = (
                rot_table.get::<f32>("x"),
                rot_table.get::<f32>("y"),
                rot_table.get::<f32>("z"),
                rot_table.get::<f32>("w"),
            ) {
                script_result.rotation = Some(Quat::from_xyzw(x, y, z, w));
            }
        }

        // scale 파싱
        if let Ok(scale_table) = table.get::<Table>("scale") {
            let x: f32 = scale_table.get("x").unwrap_or(1.0);
            let y: f32 = scale_table.get("y").unwrap_or(1.0);
            let z: f32 = scale_table.get("z").unwrap_or(1.0);
            script_result.scale = Some(Vec3::new(x, y, z));
        }

        script_result
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

    // ============ Sandbox/Validation API ============

    /// 현재 신뢰 레벨 반환
    pub fn trust_level(&self) -> TrustLevel {
        self.trust_level
    }

    /// 검증 활성화/비활성화
    pub fn set_validation_enabled(&mut self, enabled: bool) {
        self.validation_enabled = enabled;
    }

    /// 검증 활성화 여부
    pub fn is_validation_enabled(&self) -> bool {
        self.validation_enabled
    }

    /// 에러 리포터 참조
    pub fn error_reporter(&self) -> &ErrorReporter {
        &self.error_reporter
    }

    /// 에러 리포터 가변 참조
    pub fn error_reporter_mut(&mut self) -> &mut ErrorReporter {
        &mut self.error_reporter
    }

    /// 최근 에러 가져오기
    pub fn recent_errors(&self) -> &[LuaErrorInfo] {
        self.error_reporter.errors()
    }

    /// 에러 개수 (심각도별)
    pub fn error_count(&self, severity: ErrorSeverity) -> usize {
        self.error_reporter.count_by_severity(severity)
    }

    /// 에러 클리어
    pub fn clear_errors(&mut self) {
        self.error_reporter.clear();
    }

    /// Live Link 브로드캐스트용 에러 가져오기 (비우면서 반환)
    pub fn take_errors_for_broadcast(&mut self) -> Vec<LuaErrorInfo> {
        self.error_reporter.take_for_broadcast()
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

    /// Entity 레지스트리 업데이트 (매 프레임 호출)
    /// ECS World의 엔티티 정보를 Lua로 동기화
    pub fn update_entity_registry(
        &self,
        entities: &[(u64, String, Option<api::EntityTransform>)],
    ) -> LuaResult<()> {
        api::update_entity_registry(&self.lua, entities)
    }

    /// 디버그 드로우 큐 읽기 (매 프레임 호출)
    /// 스크립트에서 요청한 디버그 드로우 명령들을 읽어옴
    pub fn read_debug_draw_queue(&self) -> LuaResult<Vec<DebugDrawCommand>> {
        api::read_debug_draw_queue(&self.lua)
    }

    /// 임의의 Lua 코드 실행 (콘솔용)
    /// 결과를 문자열로 반환
    pub fn exec(&self, code: &str) -> LuaResult<String> {
        // 표현식으로 먼저 시도 (return 값이 있는 경우)
        let result: mlua::Value = match self.lua.load(format!("return {}", code)).eval() {
            Ok(val) => val,
            Err(_) => {
                // 표현식 실패 시 문장으로 실행
                self.lua.load(code).exec()?;
                return Ok(String::new());
            }
        };

        // 결과를 문자열로 변환
        let result_str = match result {
            mlua::Value::Nil => String::new(),
            mlua::Value::Boolean(b) => b.to_string(),
            mlua::Value::Integer(i) => i.to_string(),
            mlua::Value::Number(n) => format!("{:.4}", n),
            mlua::Value::String(s) => s.to_str()?.to_string(),
            mlua::Value::Table(_) => "[table]".to_string(),
            mlua::Value::Function(_) => "[function]".to_string(),
            _ => format!("{:?}", result),
        };

        Ok(result_str)
    }

    // ============ Spell API Helpers ============

    /// Process spell commands from Lua (매 프레임 호출)
    pub fn process_spell_commands(&self) -> LuaResult<Vec<SpellCommand>> {
        api::process_spell_commands(&self.lua)
    }

    /// Call spell's on_cast callback
    pub fn call_spell_on_cast(&self, spell_name: &str, caster_id: u64, target_pos: (f32, f32, f32)) -> LuaResult<Option<mlua::Table>> {
        api::call_spell_on_cast(&self.lua, spell_name, caster_id, target_pos)
    }

    /// Call spell's on_hit callback
    pub fn call_spell_on_hit(&self, spell_name: &str, caster_id: u64, target_id: u64) -> LuaResult<()> {
        api::call_spell_on_hit(&self.lua, spell_name, caster_id, target_id)
    }

    // ============ Trigger API Helpers ============

    /// Get all trigger definitions for Rust-side processing
    pub fn get_trigger_definitions(&self) -> LuaResult<Vec<(String, TriggerDefinition)>> {
        api::get_trigger_definitions(&self.lua)
    }

    /// Update trigger state and fire callbacks
    pub fn update_trigger_state(
        &self,
        trigger_name: &str,
        entity_id: u64,
        is_inside: bool,
        elapsed_time: f64,
    ) -> LuaResult<Option<TriggerEvent>> {
        api::update_trigger_state(&self.lua, trigger_name, entity_id, is_inside, elapsed_time)
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

/// 스크립트 실행 결과 - Transform 업데이트에 사용
#[derive(Debug, Clone, Default)]
pub struct ScriptResult {
    /// 새로운 위치 (Some이면 업데이트)
    pub position: Option<Vec3>,
    /// 새로운 회전 (Some이면 업데이트)
    pub rotation: Option<Quat>,
    /// 새로운 스케일 (Some이면 업데이트)
    pub scale: Option<Vec3>,
}

impl ScriptResult {
    /// 빈 결과 (업데이트 없음)
    pub fn empty() -> Self {
        Self::default()
    }

    /// 업데이트할 내용이 있는지 확인
    pub fn has_updates(&self) -> bool {
        self.position.is_some() || self.rotation.is_some() || self.scale.is_some()
    }
}

/// 스크립트 시스템 - ECS 시스템으로 스크립트 실행
pub fn script_update_system(
    mut script_engine: Option<NonSendMut<ScriptEngine>>,
    mut query: Query<(Entity, &mut LuaScript, Option<&mut Transform>)>,
    time: Option<Res<ecs_resources::Time>>,
) {
    let Some(ref mut engine) = script_engine else { return };
    let delta_time = time.map(|t| t.delta_seconds).unwrap_or(0.016);

    for (entity, mut script, mut transform) in query.iter_mut() {
        if !script.enabled {
            continue;
        }

        // 스크립트가 아직 로드되지 않았으면 로드
        if script.instance_id.is_none() {
            match engine.load_script(&script.path) {
                Ok(id) => {
                    script.instance_id = Some(id);

                    // on_start 호출
                    let ctx = create_context(entity, delta_time, transform.as_deref());
                    let _ = engine.call_start(id, &ctx);
                }
                Err(e) => {
                    log::info!("[Script] Error loading {:?}: {}", script.path, e);
                    script.enabled = false;
                }
            }
        }

        // on_update 호출 및 Transform 업데이트
        if let Some(id) = script.instance_id {
            let ctx = create_context(entity, delta_time, transform.as_deref());
            let result = engine.call_update(id, &ctx);

            // ScriptResult를 Transform에 적용
            if result.has_updates() {
                if let Some(ref mut t) = transform {
                    if let Some(pos) = result.position {
                        t.translation = pos;
                    }
                    if let Some(rot) = result.rotation {
                        t.rotation = rot;
                    }
                    if let Some(scale) = result.scale {
                        t.scale = scale;
                    }
                }
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

    #[test]
    fn test_script_result_empty() {
        let result = ScriptResult::empty();
        assert!(!result.has_updates());
        assert!(result.position.is_none());
        assert!(result.rotation.is_none());
        assert!(result.scale.is_none());
    }

    #[test]
    fn test_script_result_with_position() {
        let result = ScriptResult {
            position: Some(Vec3::new(1.0, 2.0, 3.0)),
            rotation: None,
            scale: None,
        };
        assert!(result.has_updates());
        assert_eq!(result.position, Some(Vec3::new(1.0, 2.0, 3.0)));
    }

    #[test]
    fn test_script_with_transform_return() {
        let engine = ScriptEngine::new().unwrap();
        engine.init_api().unwrap();

        // 간단한 스크립트 - position 반환
        let script_code = r#"
            local Test = {}
            function Test:on_start(ctx)
                self.pos = ctx.position
            end
            function Test:on_update(ctx)
                return {
                    position = { x = 10.0, y = 20.0, z = 30.0 }
                }
            end
            return Test
        "#;

        // 인라인 스크립트 로드
        let chunk = engine.lua.load(script_code);
        let script_table: Table = chunk.eval().unwrap();
        let instance_id = 999;
        engine.lua.set_named_registry_value(&format!("script_{}", instance_id), script_table).unwrap();

        // 컨텍스트 생성
        let ctx = ScriptContext::default();

        // on_update 호출 및 결과 확인
        let result = engine.call_update(instance_id, &ctx);
        assert!(result.has_updates());
        assert_eq!(result.position, Some(Vec3::new(10.0, 20.0, 30.0)));
    }

    #[test]
    fn test_script_with_rotation_axis_angle() {
        let engine = ScriptEngine::new().unwrap();
        engine.init_api().unwrap();

        // 축-각도 형식 회전 반환
        let script_code = r#"
            local Test = {}
            function Test:on_update(ctx)
                return {
                    rotation = { axis = "y", angle = 1.57 }
                }
            end
            return Test
        "#;

        let chunk = engine.lua.load(script_code);
        let script_table: Table = chunk.eval().unwrap();
        let instance_id = 998;
        engine.lua.set_named_registry_value(&format!("script_{}", instance_id), script_table).unwrap();

        let ctx = ScriptContext::default();
        let result = engine.call_update(instance_id, &ctx);

        assert!(result.has_updates());
        assert!(result.rotation.is_some());

        // Y축 기준 90도 회전 확인
        let rot = result.rotation.unwrap();
        let expected = Quat::from_axis_angle(Vec3::Y, 1.57);
        assert!((rot.x - expected.x).abs() < 0.001);
        assert!((rot.y - expected.y).abs() < 0.001);
        assert!((rot.z - expected.z).abs() < 0.001);
        assert!((rot.w - expected.w).abs() < 0.001);
    }
}
