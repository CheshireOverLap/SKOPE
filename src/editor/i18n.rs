//! SKOPE Editor Internationalization (i18n)
//!
//! 한국어/영어 다국어 지원 시스템

use std::collections::HashMap;

/// 지원 언어
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum Language {
    #[default]
    English,
    Korean,
}

impl Language {
    /// 언어 표시 이름 (해당 언어로)
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Korean => "한국어",
        }
    }

    /// 언어 코드
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Korean => "ko",
        }
    }

    /// 모든 언어 목록
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::Korean]
    }
}

/// 번역 키
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextKey {
    // === 툴바 ===
    Play,
    Stop,
    Pause,
    Move,
    Rotate,
    Scale,
    ToggleGrid,
    ToggleSnap,
    EditMode,
    PlayMode,

    // === 패널 제목 ===
    Hierarchy,
    Inspector,
    Console,
    Assets,
    AiAssistant,

    // === AI 패널 탭 ===
    Chat,
    Memory,
    Todos,
    PopOut,

    // === 뷰포트 ===
    Viewport,
    Aspect,
    Resolution,
    GameViewport,
    Free,

    // === 공통 ===
    File,
    Edit,
    View,
    Help,
    Settings,
    Language,
    Save,
    Load,
    New,
    Open,
    Close,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,

    // === 툴팁 ===
    TooltipPlay,
    TooltipStop,
    TooltipPause,
    TooltipMove,
    TooltipRotate,
    TooltipScale,
    TooltipGrid,
    TooltipSnap,
    TooltipAiPanel,
    TooltipLanguage,

    // === 인스펙터 ===
    Transform,
    Position,
    RotationEuler,
    ScaleXYZ,
    Components,
    AddComponent,
    RemoveComponent,
    NoSelection,
    SelectEntityToInspect,

    // === 콘솔 ===
    TypeCommandHelp,
    ClearConsole,

    // === 에셋 브라우저 ===
    SearchAssets,
    ImportAsset,
    RefreshAssets,
}

/// 번역 시스템
pub struct Translations {
    current_language: Language,
    strings: HashMap<(Language, TextKey), &'static str>,
}

impl Default for Translations {
    fn default() -> Self {
        Self::new()
    }
}

impl Translations {
    /// 새 번역 시스템 생성
    pub fn new() -> Self {
        let mut strings = HashMap::new();

        // ========== English ==========
        // 툴바
        strings.insert((Language::English, TextKey::Play), "Play");
        strings.insert((Language::English, TextKey::Stop), "Stop");
        strings.insert((Language::English, TextKey::Pause), "Pause");
        strings.insert((Language::English, TextKey::Move), "Move");
        strings.insert((Language::English, TextKey::Rotate), "Rotate");
        strings.insert((Language::English, TextKey::Scale), "Scale");
        strings.insert((Language::English, TextKey::ToggleGrid), "Toggle Grid");
        strings.insert((Language::English, TextKey::ToggleSnap), "Toggle Snap");
        strings.insert((Language::English, TextKey::EditMode), "EDIT");
        strings.insert((Language::English, TextKey::PlayMode), "PLAY");

        // 패널 제목
        strings.insert((Language::English, TextKey::Hierarchy), "Hierarchy");
        strings.insert((Language::English, TextKey::Inspector), "Inspector");
        strings.insert((Language::English, TextKey::Console), "Console");
        strings.insert((Language::English, TextKey::Assets), "Assets");
        strings.insert((Language::English, TextKey::AiAssistant), "AI Assistant");

        // AI 패널 탭
        strings.insert((Language::English, TextKey::Chat), "Chat");
        strings.insert((Language::English, TextKey::Memory), "Memory");
        strings.insert((Language::English, TextKey::Todos), "TODOs");
        strings.insert((Language::English, TextKey::PopOut), "Pop Out");

        // 뷰포트
        strings.insert((Language::English, TextKey::Viewport), "Viewport");
        strings.insert((Language::English, TextKey::Aspect), "Aspect");
        strings.insert((Language::English, TextKey::Resolution), "Scale");
        strings.insert((Language::English, TextKey::GameViewport), "Game Viewport");
        strings.insert((Language::English, TextKey::Free), "Free");

        // 공통
        strings.insert((Language::English, TextKey::File), "File");
        strings.insert((Language::English, TextKey::Edit), "Edit");
        strings.insert((Language::English, TextKey::View), "View");
        strings.insert((Language::English, TextKey::Help), "Help");
        strings.insert((Language::English, TextKey::Settings), "Settings");
        strings.insert((Language::English, TextKey::Language), "Language");
        strings.insert((Language::English, TextKey::Save), "Save");
        strings.insert((Language::English, TextKey::Load), "Load");
        strings.insert((Language::English, TextKey::New), "New");
        strings.insert((Language::English, TextKey::Open), "Open");
        strings.insert((Language::English, TextKey::Close), "Close");
        strings.insert((Language::English, TextKey::Undo), "Undo");
        strings.insert((Language::English, TextKey::Redo), "Redo");
        strings.insert((Language::English, TextKey::Cut), "Cut");
        strings.insert((Language::English, TextKey::Copy), "Copy");
        strings.insert((Language::English, TextKey::Paste), "Paste");
        strings.insert((Language::English, TextKey::Delete), "Delete");
        strings.insert((Language::English, TextKey::SelectAll), "Select All");

        // 툴팁
        strings.insert((Language::English, TextKey::TooltipPlay), "Play (F5)");
        strings.insert((Language::English, TextKey::TooltipStop), "Stop (F5)");
        strings.insert((Language::English, TextKey::TooltipPause), "Pause");
        strings.insert((Language::English, TextKey::TooltipMove), "Move (W)");
        strings.insert((Language::English, TextKey::TooltipRotate), "Rotate (E)");
        strings.insert((Language::English, TextKey::TooltipScale), "Scale (R)");
        strings.insert((Language::English, TextKey::TooltipGrid), "Toggle Grid (G)");
        strings.insert((Language::English, TextKey::TooltipSnap), "Toggle Snap");
        strings.insert((Language::English, TextKey::TooltipAiPanel), "Toggle AI Assistant");
        strings.insert((Language::English, TextKey::TooltipLanguage), "Change Language");

        // 인스펙터
        strings.insert((Language::English, TextKey::Transform), "Transform");
        strings.insert((Language::English, TextKey::Position), "Position");
        strings.insert((Language::English, TextKey::RotationEuler), "Rotation");
        strings.insert((Language::English, TextKey::ScaleXYZ), "Scale");
        strings.insert((Language::English, TextKey::Components), "Components");
        strings.insert((Language::English, TextKey::AddComponent), "Add Component");
        strings.insert((Language::English, TextKey::RemoveComponent), "Remove");
        strings.insert((Language::English, TextKey::NoSelection), "No Selection");
        strings.insert((Language::English, TextKey::SelectEntityToInspect), "Select an entity to inspect");

        // 콘솔
        strings.insert((Language::English, TextKey::TypeCommandHelp), "Type 'help' for commands");
        strings.insert((Language::English, TextKey::ClearConsole), "Clear");

        // 에셋 브라우저
        strings.insert((Language::English, TextKey::SearchAssets), "Search assets...");
        strings.insert((Language::English, TextKey::ImportAsset), "Import");
        strings.insert((Language::English, TextKey::RefreshAssets), "Refresh");

        // ========== Korean (한국어) ==========
        // 툴바
        strings.insert((Language::Korean, TextKey::Play), "재생");
        strings.insert((Language::Korean, TextKey::Stop), "정지");
        strings.insert((Language::Korean, TextKey::Pause), "일시정지");
        strings.insert((Language::Korean, TextKey::Move), "이동");
        strings.insert((Language::Korean, TextKey::Rotate), "회전");
        strings.insert((Language::Korean, TextKey::Scale), "크기");
        strings.insert((Language::Korean, TextKey::ToggleGrid), "그리드 전환");
        strings.insert((Language::Korean, TextKey::ToggleSnap), "스냅 전환");
        strings.insert((Language::Korean, TextKey::EditMode), "편집");
        strings.insert((Language::Korean, TextKey::PlayMode), "재생");

        // 패널 제목
        strings.insert((Language::Korean, TextKey::Hierarchy), "계층 구조");
        strings.insert((Language::Korean, TextKey::Inspector), "인스펙터");
        strings.insert((Language::Korean, TextKey::Console), "콘솔");
        strings.insert((Language::Korean, TextKey::Assets), "에셋");
        strings.insert((Language::Korean, TextKey::AiAssistant), "AI 어시스턴트");

        // AI 패널 탭
        strings.insert((Language::Korean, TextKey::Chat), "대화");
        strings.insert((Language::Korean, TextKey::Memory), "메모리");
        strings.insert((Language::Korean, TextKey::Todos), "할 일");
        strings.insert((Language::Korean, TextKey::PopOut), "분리");

        // 뷰포트
        strings.insert((Language::Korean, TextKey::Viewport), "뷰포트");
        strings.insert((Language::Korean, TextKey::Aspect), "비율");
        strings.insert((Language::Korean, TextKey::Resolution), "스케일");
        strings.insert((Language::Korean, TextKey::GameViewport), "게임 뷰포트");
        strings.insert((Language::Korean, TextKey::Free), "자유");

        // 공통
        strings.insert((Language::Korean, TextKey::File), "파일");
        strings.insert((Language::Korean, TextKey::Edit), "편집");
        strings.insert((Language::Korean, TextKey::View), "보기");
        strings.insert((Language::Korean, TextKey::Help), "도움말");
        strings.insert((Language::Korean, TextKey::Settings), "설정");
        strings.insert((Language::Korean, TextKey::Language), "언어");
        strings.insert((Language::Korean, TextKey::Save), "저장");
        strings.insert((Language::Korean, TextKey::Load), "불러오기");
        strings.insert((Language::Korean, TextKey::New), "새로 만들기");
        strings.insert((Language::Korean, TextKey::Open), "열기");
        strings.insert((Language::Korean, TextKey::Close), "닫기");
        strings.insert((Language::Korean, TextKey::Undo), "실행 취소");
        strings.insert((Language::Korean, TextKey::Redo), "다시 실행");
        strings.insert((Language::Korean, TextKey::Cut), "잘라내기");
        strings.insert((Language::Korean, TextKey::Copy), "복사");
        strings.insert((Language::Korean, TextKey::Paste), "붙여넣기");
        strings.insert((Language::Korean, TextKey::Delete), "삭제");
        strings.insert((Language::Korean, TextKey::SelectAll), "모두 선택");

        // 툴팁
        strings.insert((Language::Korean, TextKey::TooltipPlay), "재생 (F5)");
        strings.insert((Language::Korean, TextKey::TooltipStop), "정지 (F5)");
        strings.insert((Language::Korean, TextKey::TooltipPause), "일시정지");
        strings.insert((Language::Korean, TextKey::TooltipMove), "이동 (W)");
        strings.insert((Language::Korean, TextKey::TooltipRotate), "회전 (E)");
        strings.insert((Language::Korean, TextKey::TooltipScale), "크기 조절 (R)");
        strings.insert((Language::Korean, TextKey::TooltipGrid), "그리드 전환 (G)");
        strings.insert((Language::Korean, TextKey::TooltipSnap), "스냅 전환");
        strings.insert((Language::Korean, TextKey::TooltipAiPanel), "AI 어시스턴트 전환");
        strings.insert((Language::Korean, TextKey::TooltipLanguage), "언어 변경");

        // 인스펙터
        strings.insert((Language::Korean, TextKey::Transform), "트랜스폼");
        strings.insert((Language::Korean, TextKey::Position), "위치");
        strings.insert((Language::Korean, TextKey::RotationEuler), "회전");
        strings.insert((Language::Korean, TextKey::ScaleXYZ), "크기");
        strings.insert((Language::Korean, TextKey::Components), "컴포넌트");
        strings.insert((Language::Korean, TextKey::AddComponent), "컴포넌트 추가");
        strings.insert((Language::Korean, TextKey::RemoveComponent), "제거");
        strings.insert((Language::Korean, TextKey::NoSelection), "선택 없음");
        strings.insert((Language::Korean, TextKey::SelectEntityToInspect), "검사할 엔티티를 선택하세요");

        // 콘솔
        strings.insert((Language::Korean, TextKey::TypeCommandHelp), "'help'를 입력하여 명령어 확인");
        strings.insert((Language::Korean, TextKey::ClearConsole), "지우기");

        // 에셋 브라우저
        strings.insert((Language::Korean, TextKey::SearchAssets), "에셋 검색...");
        strings.insert((Language::Korean, TextKey::ImportAsset), "가져오기");
        strings.insert((Language::Korean, TextKey::RefreshAssets), "새로고침");

        Self {
            current_language: Language::English,
            strings,
        }
    }

    /// 현재 언어 가져오기
    pub fn language(&self) -> Language {
        self.current_language
    }

    /// 언어 설정
    pub fn set_language(&mut self, lang: Language) {
        self.current_language = lang;
        log::info!("[i18n] Language changed to: {}", lang.display_name());
    }

    /// 번역 문자열 가져오기
    pub fn get(&self, key: TextKey) -> &'static str {
        self.strings
            .get(&(self.current_language, key))
            .copied()
            .unwrap_or_else(|| {
                // 현재 언어에 없으면 영어로 폴백
                self.strings
                    .get(&(Language::English, key))
                    .copied()
                    .unwrap_or("???")
            })
    }

    /// 단축 메서드: t("key") 스타일
    pub fn t(&self, key: TextKey) -> &'static str {
        self.get(key)
    }
}

/// 전역 번역 인스턴스 (간편 접근용)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translations() {
        let mut tr = Translations::new();

        // 영어 테스트
        assert_eq!(tr.get(TextKey::Play), "Play");
        assert_eq!(tr.get(TextKey::Hierarchy), "Hierarchy");

        // 한국어로 변경
        tr.set_language(Language::Korean);
        assert_eq!(tr.get(TextKey::Play), "재생");
        assert_eq!(tr.get(TextKey::Hierarchy), "계층 구조");
    }
}
