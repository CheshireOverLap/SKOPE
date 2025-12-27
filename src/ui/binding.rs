// SKOPE UI - Data Binding System
//
// ${variable} 문법으로 동적 데이터 바인딩 지원
// 예: "${player.health}", "${inventory.gold}"
#![allow(dead_code)]

use super::types::*;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 바인딩 값 타입
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BindingValue {
    String(String),
    Number(f64),
    Bool(bool),
    Object(HashMap<String, BindingValue>),
    Array(Vec<BindingValue>),
    Null,
}

impl BindingValue {
    pub fn as_string(&self) -> String {
        match self {
            BindingValue::String(s) => s.clone(),
            BindingValue::Number(n) => format!("{}", n),
            BindingValue::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            BindingValue::Null => "".to_string(),
            BindingValue::Object(_) => "[object]".to_string(),
            BindingValue::Array(_) => "[array]".to_string(),
        }
    }

    pub fn as_f32(&self) -> f32 {
        match self {
            BindingValue::Number(n) => *n as f32,
            BindingValue::String(s) => s.parse().unwrap_or(0.0),
            BindingValue::Bool(b) => if *b { 1.0 } else { 0.0 },
            _ => 0.0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            BindingValue::Bool(b) => *b,
            BindingValue::Number(n) => *n != 0.0,
            BindingValue::String(s) => !s.is_empty() && s != "false" && s != "0",
            BindingValue::Null => false,
            _ => true,
        }
    }
}

impl From<String> for BindingValue {
    fn from(s: String) -> Self {
        BindingValue::String(s)
    }
}

impl From<&str> for BindingValue {
    fn from(s: &str) -> Self {
        BindingValue::String(s.to_string())
    }
}

impl From<f64> for BindingValue {
    fn from(n: f64) -> Self {
        BindingValue::Number(n)
    }
}

impl From<f32> for BindingValue {
    fn from(n: f32) -> Self {
        BindingValue::Number(n as f64)
    }
}

impl From<i32> for BindingValue {
    fn from(n: i32) -> Self {
        BindingValue::Number(n as f64)
    }
}

impl From<bool> for BindingValue {
    fn from(b: bool) -> Self {
        BindingValue::Bool(b)
    }
}

/// 바인딩 컨텍스트 - 모든 바인딩 변수 저장
#[derive(Debug, Default)]
pub struct BindingContext {
    /// 바인딩 변수들 (점 표기법 지원: "player.health")
    values: HashMap<String, BindingValue>,
    /// 변경된 키들 (최적화용)
    dirty_keys: Vec<String>,
}

impl BindingContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// 값 설정
    pub fn set(&mut self, key: &str, value: BindingValue) {
        self.values.insert(key.to_string(), value);
        self.dirty_keys.push(key.to_string());
    }

    /// 중첩 객체 설정 (player.health = 100)
    pub fn set_nested(&mut self, path: &str, value: BindingValue) {
        let parts: Vec<&str> = path.split('.').collect();

        if parts.len() == 1 {
            self.set(path, value);
            return;
        }

        // 루트 객체 가져오기 또는 생성
        let root_key = parts[0];
        let mut current = self.values
            .entry(root_key.to_string())
            .or_insert_with(|| BindingValue::Object(HashMap::new()))
            .clone();

        // 중첩 경로 탐색 및 설정
        set_nested_value(&mut current, &parts[1..], value);

        self.values.insert(root_key.to_string(), current);
        self.dirty_keys.push(path.to_string());
    }

    /// 값 가져오기
    pub fn get(&self, key: &str) -> Option<&BindingValue> {
        // 단순 키
        if let Some(value) = self.values.get(key) {
            return Some(value);
        }

        // 점 표기법 처리
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() > 1 {
            if let Some(root) = self.values.get(parts[0]) {
                return get_nested_value(root, &parts[1..]);
            }
        }

        None
    }

    /// 문자열의 바인딩 표현식 평가
    /// "${player.health}" -> "100"
    pub fn evaluate_string(&self, text: &str) -> String {
        let mut result = text.to_string();

        // ${...} 패턴 찾기
        while let Some(start) = result.find("${") {
            if let Some(end) = result[start..].find('}') {
                let end = start + end;
                let expr = &result[start + 2..end];

                let value = self.evaluate_expression(expr);
                result = format!("{}{}{}", &result[..start], value, &result[end + 1..]);
            } else {
                break;
            }
        }

        result
    }

    /// 표현식 평가
    pub fn evaluate_expression(&self, expr: &str) -> String {
        let expr = expr.trim();

        // 간단한 변수 참조
        if let Some(value) = self.get(expr) {
            return value.as_string();
        }

        // 간단한 연산 지원 (추후 확장 가능)
        if let Some(result) = self.evaluate_simple_math(expr) {
            return format!("{}", result);
        }

        // 찾지 못함
        format!("${{{}}}", expr)
    }

    /// 간단한 수학 연산
    fn evaluate_simple_math(&self, expr: &str) -> Option<f32> {
        // "value * 100" 형태
        if let Some(pos) = expr.find('*') {
            let left = expr[..pos].trim();
            let right = expr[pos + 1..].trim();

            let left_val = self.get(left)?.as_f32();
            let right_val: f32 = right.parse().ok()?;

            return Some(left_val * right_val);
        }

        // "value / 100" 형태
        if let Some(pos) = expr.find('/') {
            let left = expr[..pos].trim();
            let right = expr[pos + 1..].trim();

            let left_val = self.get(left)?.as_f32();
            let right_val: f32 = right.parse().ok()?;

            if right_val != 0.0 {
                return Some(left_val / right_val);
            }
        }

        // "value + offset" 형태
        if let Some(pos) = expr.find('+') {
            let left = expr[..pos].trim();
            let right = expr[pos + 1..].trim();

            let left_val = self.get(left)?.as_f32();
            let right_val: f32 = right.parse().ok()?;

            return Some(left_val + right_val);
        }

        // "value - offset" 형태
        if let Some(pos) = expr.rfind('-') {
            if pos > 0 {  // 음수 숫자가 아닌 경우만
                let left = expr[..pos].trim();
                let right = expr[pos + 1..].trim();

                if let Some(left_val) = self.get(left) {
                    if let Ok(right_val) = right.parse::<f32>() {
                        return Some(left_val.as_f32() - right_val);
                    }
                }
            }
        }

        None
    }

    /// 위젯의 바인딩 업데이트
    pub fn update_widget(&self, widget: &mut Widget) {
        // 위젯 타입별 바인딩 처리
        match &mut widget.widget_type {
            WidgetType::Text { ref mut content, .. } => {
                if content.contains("${") {
                    *content = self.evaluate_string(content);
                }
            }
            WidgetType::ProgressBar { value: _, max_value: _, .. } => {
                // value가 바인딩 표현식인 경우 처리 (실제로는 별도 필드 필요)
                // 여기서는 예시
            }
            _ => {}
        }

        // 이벤트 핸들러의 바인딩 처리
        for (_, handler) in &widget.events {
            if handler.contains("${") {
                // 이벤트 핸들러 내 바인딩 처리
            }
        }

        // 자식 위젯들도 업데이트
        for child in &mut widget.children {
            self.update_widget(child);
        }
    }

    /// 변경된 키 목록 반환 및 초기화
    pub fn take_dirty_keys(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dirty_keys)
    }

    /// 모든 값 초기화
    pub fn clear(&mut self) {
        self.values.clear();
        self.dirty_keys.clear();
    }
}

/// 중첩 객체에서 값 가져오기
fn get_nested_value<'a>(value: &'a BindingValue, path: &[&str]) -> Option<&'a BindingValue> {
    if path.is_empty() {
        return Some(value);
    }

    match value {
        BindingValue::Object(map) => {
            map.get(path[0]).and_then(|v| get_nested_value(v, &path[1..]))
        }
        BindingValue::Array(arr) => {
            if let Ok(idx) = path[0].parse::<usize>() {
                arr.get(idx).and_then(|v| get_nested_value(v, &path[1..]))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// 중첩 객체에 값 설정
fn set_nested_value(value: &mut BindingValue, path: &[&str], new_value: BindingValue) {
    if path.is_empty() {
        *value = new_value;
        return;
    }

    if let BindingValue::Object(ref mut map) = value {
        let key = path[0].to_string();

        if path.len() == 1 {
            map.insert(key, new_value);
        } else {
            let entry = map.entry(key)
                .or_insert_with(|| BindingValue::Object(HashMap::new()));
            set_nested_value(entry, &path[1..], new_value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_binding() {
        let mut ctx = BindingContext::new();
        ctx.set("name", BindingValue::String("Player".to_string()));

        assert_eq!(
            ctx.evaluate_string("Hello, ${name}!"),
            "Hello, Player!"
        );
    }

    #[test]
    fn test_nested_binding() {
        let mut ctx = BindingContext::new();

        let mut player = HashMap::new();
        player.insert("health".to_string(), BindingValue::Number(100.0));
        player.insert("name".to_string(), BindingValue::String("Hero".to_string()));

        ctx.set("player", BindingValue::Object(player));

        assert_eq!(
            ctx.evaluate_string("${player.name}: ${player.health} HP"),
            "Hero: 100 HP"
        );
    }

    #[test]
    fn test_math_expression() {
        let mut ctx = BindingContext::new();
        ctx.set("value", BindingValue::Number(0.75));

        assert_eq!(
            ctx.evaluate_expression("value * 100"),
            "75"
        );
    }

    #[test]
    fn test_set_nested() {
        let mut ctx = BindingContext::new();
        ctx.set_nested("player.health", BindingValue::Number(80.0));

        assert_eq!(
            ctx.get("player.health").unwrap().as_f32(),
            80.0
        );
    }
}
