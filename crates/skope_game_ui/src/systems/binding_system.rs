//! SKOPE UI - Binding System
//!
//! 데이터 바인딩 관리

use crate::types::Widget;
use crate::binding::{BindingContext, BindingValue};

/// 바인딩 시스템
pub struct BindingSystem {
    /// 바인딩 컨텍스트
    context: BindingContext,
}

impl BindingSystem {
    pub fn new() -> Self {
        Self {
            context: BindingContext::new(),
        }
    }

    /// 값 설정
    pub fn set(&mut self, key: &str, value: BindingValue) {
        self.context.set(key, value);
    }

    /// 중첩 값 설정 (예: "player.health")
    pub fn set_nested(&mut self, path: &str, value: BindingValue) {
        self.context.set_nested(path, value);
    }

    /// 값 가져오기
    pub fn get(&self, key: &str) -> Option<&BindingValue> {
        self.context.get(key)
    }

    /// 문자열 내 바인딩 표현식 평가
    pub fn evaluate_string(&self, text: &str) -> String {
        self.context.evaluate_string(text)
    }

    /// 위젯의 바인딩 업데이트
    pub fn update_widget(&self, widget: &mut Widget) {
        self.context.update_widget(widget);
    }

    /// 변경된 키 목록 반환
    pub fn take_dirty_keys(&mut self) -> Vec<String> {
        self.context.take_dirty_keys()
    }

    /// 모든 값 초기화
    pub fn clear(&mut self) {
        self.context.clear();
    }

    /// 내부 컨텍스트 참조 (호환성용)
    pub fn context(&self) -> &BindingContext {
        &self.context
    }

    /// 내부 컨텍스트 가변 참조 (호환성용)
    pub fn context_mut(&mut self) -> &mut BindingContext {
        &mut self.context
    }
}

impl Default for BindingSystem {
    fn default() -> Self {
        Self::new()
    }
}
