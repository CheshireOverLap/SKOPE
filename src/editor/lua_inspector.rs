//! Lua Inspector - Lua 스크립트 변수 시각화 및 편집
//!
//! Inspector에서 값 수정 → Lua 파일 직접 수정 → Hot Reload 자동 반영

use egui::{Color32, Ui, DragValue};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Lua 변수 타입
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Vec3(f32, f32, f32),
}

impl LuaValue {
    /// Lua 코드 문자열로 변환
    pub fn to_lua_string(&self) -> String {
        match self {
            LuaValue::Number(n) => {
                // 정수면 소수점 없이
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            LuaValue::String(s) => format!("\"{}\"", s),
            LuaValue::Boolean(b) => if *b { "true".to_string() } else { "false".to_string() },
            LuaValue::Vec3(x, y, z) => format!("{{x={}, y={}, z={}}}", x, y, z),
        }
    }
}

/// 파싱된 Lua 변수
#[derive(Debug, Clone)]
pub struct LuaVariable {
    /// 변수 이름 (예: "speed", "health")
    pub name: String,
    /// 현재 값
    pub value: LuaValue,
    /// 파일 내 라인 번호 (1-based)
    pub line_number: usize,
    /// 원본 라인 텍스트
    pub original_line: String,
}

/// Lua 스크립트 정보
#[derive(Debug, Clone)]
pub struct LuaScriptInfo {
    /// 파일 경로
    pub path: PathBuf,
    /// 모듈 이름 (예: "Rotator")
    pub module_name: String,
    /// 편집 가능한 변수들
    pub variables: Vec<LuaVariable>,
}

/// Lua Inspector 상태
pub struct LuaInspectorState {
    /// 현재 로드된 스크립트 정보
    current_script: Option<LuaScriptInfo>,
    /// 편집 중인 값들 (변수 이름 → 편집 중인 문자열)
    editing: HashMap<String, String>,
    /// Vec3 편집 상태 (변수 이름 → (x, y, z))
    editing_vec3: HashMap<String, (f32, f32, f32)>,
    /// 마지막 로드한 경로
    last_path: Option<PathBuf>,
}

impl Default for LuaInspectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl LuaInspectorState {
    pub fn new() -> Self {
        Self {
            current_script: None,
            editing: HashMap::new(),
            editing_vec3: HashMap::new(),
            last_path: None,
        }
    }

    /// Lua 파일 로드 및 파싱
    pub fn load_script(&mut self, path: &Path) {
        // 이미 로드된 경우 스킵
        if self.last_path.as_ref() == Some(&path.to_path_buf()) && self.current_script.is_some() {
            return;
        }

        self.last_path = Some(path.to_path_buf());
        self.editing.clear();
        self.editing_vec3.clear();

        match fs::read_to_string(path) {
            Ok(content) => {
                self.current_script = Some(parse_lua_script(path, &content));
            }
            Err(e) => {
                log::warn!("[LuaInspector] Failed to read {}: {}", path.display(), e);
                self.current_script = None;
            }
        }
    }

    /// 강제 리로드
    pub fn reload(&mut self) {
        if let Some(path) = self.last_path.clone() {
            self.last_path = None; // 강제 리로드
            self.load_script(&path);
        }
    }

    /// Inspector UI 렌더링
    /// 반환: 파일이 수정되었으면 true
    pub fn ui(&mut self, ui: &mut Ui, script_path: &Path) -> bool {
        // 스크립트 로드
        self.load_script(script_path);

        let Some(script_info) = &self.current_script else {
            ui.label(egui::RichText::new("Failed to load script")
                .color(Color32::from_rgb(200, 100, 100)));
            return false;
        };

        let mut file_modified = false;
        let module_name = script_info.module_name.clone();
        let path = script_info.path.clone();
        let variables = script_info.variables.clone();

        // 헤더
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("📜 {}", module_name))
                .strong()
                .color(Color32::from_rgb(180, 220, 180)));
        });

        ui.add_space(4.0);

        // 변수들 표시
        if variables.is_empty() {
            ui.label(egui::RichText::new("No editable variables found")
                .italics()
                .color(Color32::from_rgb(120, 120, 130)));
        } else {
            for var in &variables {
                if let Some(new_value) = self.render_variable(ui, var) {
                    // 값이 변경됨 → Lua 파일 수정
                    if modify_lua_file(&path, var, &new_value) {
                        file_modified = true;
                        log::info!("[LuaInspector] Modified {}.{} = {}",
                            module_name, var.name, new_value.to_lua_string());
                    }
                }
            }
        }

        // 파일 수정 후 리로드
        if file_modified {
            self.last_path = None; // 다음 프레임에 리로드
        }

        file_modified
    }

    /// 개별 변수 UI 렌더링
    /// 반환: 새 값 (변경되었을 때만)
    fn render_variable(&mut self, ui: &mut Ui, var: &LuaVariable) -> Option<LuaValue> {
        let mut result = None;

        ui.horizontal(|ui| {
            ui.add_space(8.0);

            // 라벨
            ui.label(egui::RichText::new(&var.name)
                .size(11.0)
                .color(Color32::from_rgb(160, 165, 175)));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match &var.value {
                    LuaValue::Number(n) => {
                        // 숫자 편집
                        let edit_key = var.name.clone();
                        let mut edit_str = self.editing
                            .entry(edit_key.clone())
                            .or_insert_with(|| format!("{}", n))
                            .clone();

                        let response = ui.add(
                            egui::TextEdit::singleline(&mut edit_str)
                                .desired_width(80.0)
                                .font(egui::TextStyle::Monospace)
                        );

                        if response.changed() {
                            self.editing.insert(edit_key.clone(), edit_str.clone());
                        }

                        if response.lost_focus() {
                            if let Ok(new_n) = edit_str.parse::<f64>() {
                                if (new_n - n).abs() > f64::EPSILON {
                                    result = Some(LuaValue::Number(new_n));
                                }
                            }
                            // 포커스 잃으면 편집 상태 초기화
                            self.editing.remove(&edit_key);
                        }
                    }
                    LuaValue::String(s) => {
                        // 문자열 편집
                        let edit_key = var.name.clone();
                        let mut edit_str = self.editing
                            .entry(edit_key.clone())
                            .or_insert_with(|| s.clone())
                            .clone();

                        let response = ui.add(
                            egui::TextEdit::singleline(&mut edit_str)
                                .desired_width(120.0)
                                .font(egui::TextStyle::Monospace)
                        );

                        if response.changed() {
                            self.editing.insert(edit_key.clone(), edit_str.clone());
                        }

                        if response.lost_focus() && edit_str != *s {
                            result = Some(LuaValue::String(edit_str));
                            self.editing.remove(&edit_key);
                        }
                    }
                    LuaValue::Boolean(b) => {
                        // 체크박스
                        let mut checked = *b;
                        if ui.checkbox(&mut checked, "").changed() {
                            result = Some(LuaValue::Boolean(checked));
                        }
                    }
                    LuaValue::Vec3(_, _, _) => {
                        // Vec3는 별도 처리 (아래 horizontal에서)
                    }
                }
            });
        });

        // Vec3 전용 UI (별도 행)
        if let LuaValue::Vec3(x, y, z) = &var.value {
            let edit_key = format!("{}_vec3", var.name);

            // 편집 상태 초기화
            let (mut ex, mut ey, mut ez) = *self.editing_vec3
                .entry(edit_key.clone())
                .or_insert((*x, *y, *z));

            let mut changed = false;

            ui.horizontal(|ui| {
                ui.add_space(16.0);

                // X (빨강)
                ui.label(egui::RichText::new("X").size(10.0).color(Color32::from_rgb(220, 80, 80)));
                let x_resp = ui.add(
                    DragValue::new(&mut ex)
                        .speed(0.01)
                        .min_decimals(2)
                        .max_decimals(3)
                );
                changed |= x_resp.changed();

                ui.add_space(4.0);

                // Y (초록)
                ui.label(egui::RichText::new("Y").size(10.0).color(Color32::from_rgb(80, 200, 80)));
                let y_resp = ui.add(
                    DragValue::new(&mut ey)
                        .speed(0.01)
                        .min_decimals(2)
                        .max_decimals(3)
                );
                changed |= y_resp.changed();

                ui.add_space(4.0);

                // Z (파랑)
                ui.label(egui::RichText::new("Z").size(10.0).color(Color32::from_rgb(80, 140, 220)));
                let z_resp = ui.add(
                    DragValue::new(&mut ez)
                        .speed(0.01)
                        .min_decimals(2)
                        .max_decimals(3)
                );
                changed |= z_resp.changed();
            });

            if changed {
                self.editing_vec3.insert(edit_key, (ex, ey, ez));
                result = Some(LuaValue::Vec3(ex, ey, ez));
            }
        }

        result
    }
}

/// Lua 스크립트 파싱
fn parse_lua_script(path: &Path, content: &str) -> LuaScriptInfo {
    let mut module_name = path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();

    // 첫 글자 대문자로 (관례)
    if let Some(first) = module_name.chars().next() {
        module_name = first.to_uppercase().to_string() + &module_name[1..];
    }

    let mut variables = Vec::new();

    // 패턴: ModuleName.property = value
    // 예: Rotator.speed = 45.0
    let pattern = format!(r"^(\s*){}\s*\.\s*(\w+)\s*=\s*(.+)", regex::escape(&module_name));
    let re = regex::Regex::new(&pattern).ok();

    for (line_idx, line) in content.lines().enumerate() {
        let line_number = line_idx + 1;

        // 주석 라인 스킵
        let trimmed = line.trim();
        if trimmed.starts_with("--") {
            continue;
        }

        // 정규식 매칭
        if let Some(ref re) = re {
            if let Some(caps) = re.captures(line) {
                let var_name = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                let value_str = caps.get(3).map(|m| m.as_str()).unwrap_or("").trim();

                // 주석 제거
                let value_str = value_str.split("--").next().unwrap_or("").trim();

                // 값 파싱
                if let Some(value) = parse_lua_value(value_str) {
                    // 런타임 상태 변수 및 콜백 제외
                    const EXCLUDED_PREFIXES: &[&str] = &[
                        "on_",      // 콜백 함수
                        "_",        // Private 변수
                        "current_", // 현재 상태 값
                        "last_",    // 이전 프레임 값
                        "prev_",    // 이전 값
                        "is_",      // 상태 플래그
                        "has_",     // 상태 플래그
                    ];

                    let is_excluded = EXCLUDED_PREFIXES.iter()
                        .any(|prefix| var_name.starts_with(prefix));

                    if !is_excluded {
                        variables.push(LuaVariable {
                            name: var_name.to_string(),
                            value,
                            line_number,
                            original_line: line.to_string(),
                        });
                    }
                }
            }
        }
    }

    LuaScriptInfo {
        path: path.to_path_buf(),
        module_name,
        variables,
    }
}

/// Lua 값 문자열 파싱
fn parse_lua_value(s: &str) -> Option<LuaValue> {
    let s = s.trim();

    // Boolean
    if s == "true" {
        return Some(LuaValue::Boolean(true));
    }
    if s == "false" {
        return Some(LuaValue::Boolean(false));
    }

    // Number
    if let Ok(n) = s.parse::<f64>() {
        return Some(LuaValue::Number(n));
    }

    // String (쌍따옴표)
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return Some(LuaValue::String(s[1..s.len()-1].to_string()));
    }

    // String (홑따옴표)
    if s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2 {
        return Some(LuaValue::String(s[1..s.len()-1].to_string()));
    }

    // Vec3 테이블: {x=0, y=1, z=0}
    if s.starts_with('{') && s.ends_with('}') && s.len() >= 2 {
        if let Some(vec3) = parse_vec3_table(&s[1..s.len()-1]) {
            return Some(vec3);
        }
    }

    None
}

/// Vec3 테이블 파싱: x=0, y=1, z=0
fn parse_vec3_table(content: &str) -> Option<LuaValue> {
    let mut x: Option<f32> = None;
    let mut y: Option<f32> = None;
    let mut z: Option<f32> = None;

    for pair in content.split(',') {
        let pair = pair.trim();
        if let Some(eq_pos) = pair.find('=') {
            let key = pair[..eq_pos].trim();
            let value_str = pair[eq_pos + 1..].trim();

            if let Ok(n) = value_str.parse::<f32>() {
                match key {
                    "x" => x = Some(n),
                    "y" => y = Some(n),
                    "z" => z = Some(n),
                    _ => {}
                }
            }
        }
    }

    match (x, y, z) {
        (Some(x), Some(y), Some(z)) => Some(LuaValue::Vec3(x, y, z)),
        _ => None,
    }
}

/// Lua 파일 수정
fn modify_lua_file(path: &Path, var: &LuaVariable, new_value: &LuaValue) -> bool {
    // 파일 읽기
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("[LuaInspector] Failed to read file: {}", e);
            return false;
        }
    };

    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    // 라인 번호 확인 (1-based)
    if var.line_number == 0 || var.line_number > lines.len() {
        log::error!("[LuaInspector] Invalid line number: {}", var.line_number);
        return false;
    }

    let line_idx = var.line_number - 1;
    let original_line = &lines[line_idx];

    // 값 부분만 교체
    // 패턴: (앞부분)(변수이름 = )(값)(뒷부분: 주석 등)
    if let Some(eq_pos) = original_line.find('=') {
        let before_eq = &original_line[..=eq_pos]; // "Rotator.speed = " 부분 포함

        // 주석 분리
        let after_eq = &original_line[eq_pos + 1..];
        let (_, comment) = if let Some(comment_pos) = after_eq.find("--") {
            let value_part = &after_eq[..comment_pos];
            let comment_part = &after_eq[comment_pos..];
            (value_part, Some(comment_part))
        } else {
            (after_eq, None)
        };

        // 새 라인 생성
        let new_line = if let Some(comment) = comment {
            format!("{} {}{}", before_eq, new_value.to_lua_string(), comment)
        } else {
            format!("{} {}", before_eq, new_value.to_lua_string())
        };

        lines[line_idx] = new_line;

        // 파일 쓰기
        let new_content = lines.join("\n");
        match fs::write(path, new_content) {
            Ok(_) => {
                log::info!("[LuaInspector] Saved: {}", path.display());
                return true;
            }
            Err(e) => {
                log::error!("[LuaInspector] Failed to write file: {}", e);
                return false;
            }
        }
    }

    false
}
