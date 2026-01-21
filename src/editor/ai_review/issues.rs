//! AI Code Review Issues
//!
//! 코드 리뷰 이슈 타입 및 관리 시스템

use std::path::PathBuf;
use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};

/// 이슈 심각도
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// 에러 - 반드시 수정 필요
    Error,
    /// 경고 - 수정 권장
    Warning,
    /// 정보 - 참고용
    Info,
    /// 힌트 - 개선 제안
    Hint,
}

impl IssueSeverity {
    /// 아이콘 문자
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Error => "X",
            Self::Warning => "!",
            Self::Info => "i",
            Self::Hint => "?",
        }
    }

    /// 색상 (egui Color32 형태)
    pub fn color(&self) -> [u8; 3] {
        match self {
            Self::Error => [255, 80, 80],
            Self::Warning => [255, 200, 80],
            Self::Info => [100, 180, 255],
            Self::Hint => [180, 180, 180],
        }
    }

    /// 표시 이름
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Info => "Info",
            Self::Hint => "Hint",
        }
    }
}

/// 이슈 카테고리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IssueCategory {
    /// 문법 오류
    Syntax,
    /// 런타임 에러 가능성
    Runtime,
    /// 성능 이슈
    Performance,
    /// 보안 취약점
    Security,
    /// 베스트 프랙티스 위반
    BestPractice,
    /// 사용하지 않는 코드
    UnusedCode,
    /// 타입 관련
    Type,
    /// 스타일/포맷팅
    Style,
    /// 기타
    Other,
}

impl IssueCategory {
    /// 표시 이름
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Syntax => "Syntax",
            Self::Runtime => "Runtime",
            Self::Performance => "Performance",
            Self::Security => "Security",
            Self::BestPractice => "Best Practice",
            Self::UnusedCode => "Unused Code",
            Self::Type => "Type",
            Self::Style => "Style",
            Self::Other => "Other",
        }
    }
}

/// 텍스트 범위
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextRange {
    /// 시작 라인 (1-based)
    pub start_line: usize,
    /// 시작 컬럼 (1-based)
    pub start_column: usize,
    /// 종료 라인 (1-based)
    pub end_line: usize,
    /// 종료 컬럼 (1-based)
    pub end_column: usize,
}

impl TextRange {
    /// 단일 라인 범위 생성
    pub fn single_line(line: usize) -> Self {
        Self {
            start_line: line,
            start_column: 1,
            end_line: line,
            end_column: usize::MAX,
        }
    }

    /// 여러 라인 범위 생성
    pub fn lines(start: usize, end: usize) -> Self {
        Self {
            start_line: start,
            start_column: 1,
            end_line: end,
            end_column: usize::MAX,
        }
    }
}

/// 코드 수정 제안
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFix {
    /// 수정 설명
    pub description: String,
    /// 대체 텍스트
    pub replacement: String,
    /// 수정 범위
    pub range: TextRange,
    /// 수정 후 추가 변경 필요 여부
    pub needs_review: bool,
}

impl CodeFix {
    /// 간단한 대체 수정 생성
    pub fn simple_replacement(description: &str, replacement: &str, line: usize) -> Self {
        Self {
            description: description.to_string(),
            replacement: replacement.to_string(),
            range: TextRange::single_line(line),
            needs_review: false,
        }
    }
}

/// 코드 이슈
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeIssue {
    /// 고유 ID
    pub id: u64,
    /// 심각도
    pub severity: IssueSeverity,
    /// 카테고리
    pub category: IssueCategory,
    /// 파일 경로
    pub file_path: PathBuf,
    /// 위치 범위
    pub range: TextRange,
    /// 이슈 메시지
    pub message: String,
    /// 상세 설명
    pub explanation: Option<String>,
    /// 자동 수정 제안
    pub auto_fix: Option<CodeFix>,
    /// 관련 문서 링크
    pub doc_url: Option<String>,
    /// 무시됨 여부
    #[serde(default)]
    pub dismissed: bool,
}

impl CodeIssue {
    /// 새 이슈 생성
    pub fn new(
        severity: IssueSeverity,
        category: IssueCategory,
        file_path: PathBuf,
        line: usize,
        message: &str,
    ) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            severity,
            category,
            file_path,
            range: TextRange::single_line(line),
            message: message.to_string(),
            explanation: None,
            auto_fix: None,
            doc_url: None,
            dismissed: false,
        }
    }

    /// 설명 추가
    pub fn with_explanation(mut self, explanation: &str) -> Self {
        self.explanation = Some(explanation.to_string());
        self
    }

    /// 자동 수정 추가
    pub fn with_fix(mut self, fix: CodeFix) -> Self {
        self.auto_fix = Some(fix);
        self
    }

    /// 라인 번호
    pub fn line(&self) -> usize {
        self.range.start_line
    }
}

/// 리뷰 세션 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReviewStatus {
    /// 대기 중
    #[default]
    Idle,
    /// 분석 중
    Analyzing,
    /// 완료
    Completed,
    /// 에러
    Error,
}

/// AI 코드 리뷰 세션
#[derive(Debug, Clone)]
pub struct ReviewSession {
    /// 세션 ID
    pub id: u64,
    /// 대상 파일
    pub file_path: PathBuf,
    /// 발견된 이슈들
    pub issues: Vec<CodeIssue>,
    /// 세션 상태
    pub status: ReviewStatus,
    /// 생성 시간
    pub created_at: Instant,
    /// 분석 완료 시간
    pub completed_at: Option<Instant>,
    /// 에러 메시지 (에러 상태 시)
    pub error_message: Option<String>,
}

impl ReviewSession {
    /// 새 세션 생성
    pub fn new(file_path: PathBuf) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            file_path,
            issues: Vec::new(),
            status: ReviewStatus::Idle,
            created_at: Instant::now(),
            completed_at: None,
            error_message: None,
        }
    }

    /// 분석 시작
    pub fn start_analysis(&mut self) {
        self.status = ReviewStatus::Analyzing;
        self.issues.clear();
        self.error_message = None;
    }

    /// 분석 완료
    pub fn complete(&mut self, issues: Vec<CodeIssue>) {
        self.issues = issues;
        self.status = ReviewStatus::Completed;
        self.completed_at = Some(Instant::now());
    }

    /// 에러 설정
    pub fn set_error(&mut self, message: &str) {
        self.status = ReviewStatus::Error;
        self.error_message = Some(message.to_string());
        self.completed_at = Some(Instant::now());
    }

    /// 심각도별 이슈 수
    pub fn count_by_severity(&self, severity: IssueSeverity) -> usize {
        self.issues.iter().filter(|i| i.severity == severity && !i.dismissed).count()
    }

    /// 활성 이슈 수 (dismissed 제외)
    pub fn active_issue_count(&self) -> usize {
        self.issues.iter().filter(|i| !i.dismissed).count()
    }

    /// 분석 소요 시간
    pub fn duration(&self) -> Option<std::time::Duration> {
        self.completed_at.map(|c| c.duration_since(self.created_at))
    }
}

/// AI 코드 리뷰 관리자
#[derive(Debug, Default)]
pub struct CodeReviewManager {
    /// 파일별 리뷰 세션
    pub sessions: HashMap<PathBuf, ReviewSession>,
    /// 전역 설정: 저장 시 자동 리뷰
    pub auto_review_on_save: bool,
    /// 전역 설정: 리뷰할 파일 패턴
    pub file_patterns: Vec<String>,
    /// 전역 설정: 무시할 파일 패턴
    pub ignore_patterns: Vec<String>,
}

impl CodeReviewManager {
    /// 새 매니저 생성
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            auto_review_on_save: true,
            file_patterns: vec![
                "*.lua".to_string(),
            ],
            ignore_patterns: vec![
                "**/node_modules/**".to_string(),
                "**/target/**".to_string(),
            ],
        }
    }

    /// 파일에 대한 리뷰 요청
    pub fn request_review(&mut self, file_path: PathBuf) -> &mut ReviewSession {
        let session = self.sessions.entry(file_path.clone())
            .or_insert_with(|| ReviewSession::new(file_path));
        session.start_analysis();
        session
    }

    /// 파일의 이슈 가져오기
    pub fn get_issues(&self, file_path: &PathBuf) -> Option<&[CodeIssue]> {
        self.sessions.get(file_path).map(|s| s.issues.as_slice())
    }

    /// 라인별 이슈 가져오기
    pub fn get_issues_for_line(&self, file_path: &PathBuf, line: usize) -> Vec<&CodeIssue> {
        self.sessions.get(file_path)
            .map(|s| {
                s.issues.iter()
                    .filter(|i| !i.dismissed && i.range.start_line <= line && line <= i.range.end_line)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 이슈 무시 처리
    pub fn dismiss_issue(&mut self, file_path: &PathBuf, issue_id: u64) {
        if let Some(session) = self.sessions.get_mut(file_path) {
            if let Some(issue) = session.issues.iter_mut().find(|i| i.id == issue_id) {
                issue.dismissed = true;
            }
        }
    }

    /// 모든 이슈 수 (전체 파일)
    pub fn total_issue_count(&self) -> usize {
        self.sessions.values()
            .map(|s| s.active_issue_count())
            .sum()
    }

    /// 에러 수 (전체 파일)
    pub fn total_error_count(&self) -> usize {
        self.sessions.values()
            .map(|s| s.count_by_severity(IssueSeverity::Error))
            .sum()
    }

    /// 경고 수 (전체 파일)
    pub fn total_warning_count(&self) -> usize {
        self.sessions.values()
            .map(|s| s.count_by_severity(IssueSeverity::Warning))
            .sum()
    }
}

/// 리뷰 액션 (UI에서 발생)
#[derive(Debug, Clone)]
pub enum ReviewAction {
    /// 리뷰 요청
    RequestReview(PathBuf),
    /// 이슈 무시
    DismissIssue { file_path: PathBuf, issue_id: u64 },
    /// 자동 수정 적용
    ApplyFix { file_path: PathBuf, issue_id: u64 },
    /// 이슈로 이동
    GoToIssue { file_path: PathBuf, issue_id: u64 },
}

// ============================================================================
// Code Metrics
// ============================================================================

/// 코드 메트릭스
///
/// 스크립트 파일의 품질 지표
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CodeMetrics {
    /// 총 라인 수
    pub total_lines: usize,
    /// 코드 라인 수 (빈 줄, 주석 제외)
    pub code_lines: usize,
    /// 주석 라인 수
    pub comment_lines: usize,
    /// 빈 라인 수
    pub blank_lines: usize,
    /// 함수 수
    pub function_count: usize,
    /// 평균 함수 길이 (라인)
    pub avg_function_length: f32,
    /// 최대 함수 길이 (라인)
    pub max_function_length: usize,
    /// 순환 복잡도 (평균)
    pub avg_complexity: f32,
    /// 최대 순환 복잡도
    pub max_complexity: usize,
    /// 전역 변수 수
    pub global_count: usize,
    /// TODO 주석 수
    pub todo_count: usize,
    /// FIXME 주석 수
    pub fixme_count: usize,
}

impl CodeMetrics {
    /// 새 메트릭스 (빈 파일)
    pub fn new() -> Self {
        Self::default()
    }

    /// Lua 코드에서 메트릭스 계산
    pub fn from_lua(content: &str) -> Self {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        let mut code_lines = 0;
        let mut comment_lines = 0;
        let mut blank_lines = 0;
        let mut in_multiline_comment = false;
        let mut function_count = 0;
        let mut function_lengths: Vec<usize> = Vec::new();
        let mut current_function_start: Option<usize> = None;
        let mut global_count = 0;
        let mut todo_count = 0;
        let mut fixme_count = 0;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();

            // 빈 줄
            if trimmed.is_empty() {
                blank_lines += 1;
                continue;
            }

            // 멀티라인 주석 체크
            if in_multiline_comment {
                comment_lines += 1;
                if trimmed.contains("]]") {
                    in_multiline_comment = false;
                }
                continue;
            }

            if trimmed.starts_with("--[[") {
                in_multiline_comment = true;
                comment_lines += 1;
                continue;
            }

            // 단일 라인 주석
            if trimmed.starts_with("--") {
                comment_lines += 1;

                // TODO/FIXME 체크
                let upper = trimmed.to_uppercase();
                if upper.contains("TODO") {
                    todo_count += 1;
                }
                if upper.contains("FIXME") {
                    fixme_count += 1;
                }
                continue;
            }

            code_lines += 1;

            // 함수 시작
            if trimmed.contains("function") && (trimmed.starts_with("function") || trimmed.contains("= function")) {
                function_count += 1;
                current_function_start = Some(i);
            }

            // 함수 종료
            if trimmed == "end" && current_function_start.is_some() {
                if let Some(start) = current_function_start {
                    function_lengths.push(i - start + 1);
                }
                current_function_start = None;
            }

            // 전역 변수 (간단한 휴리스틱)
            if !trimmed.starts_with("local")
                && !trimmed.starts_with("function")
                && !trimmed.starts_with("return")
                && !trimmed.starts_with("end")
                && !trimmed.starts_with("if")
                && !trimmed.starts_with("for")
                && !trimmed.starts_with("while")
                && !trimmed.starts_with("else")
                && trimmed.contains("=")
                && !trimmed.contains("==")
            {
                global_count += 1;
            }
        }

        // 함수 길이 통계
        let avg_function_length = if function_lengths.is_empty() {
            0.0
        } else {
            function_lengths.iter().sum::<usize>() as f32 / function_lengths.len() as f32
        };
        let max_function_length = function_lengths.iter().copied().max().unwrap_or(0);

        Self {
            total_lines,
            code_lines,
            comment_lines,
            blank_lines,
            function_count,
            avg_function_length,
            max_function_length,
            avg_complexity: 1.0, // 간단한 기본값 (실제 계산은 복잡)
            max_complexity: 1,
            global_count,
            todo_count,
            fixme_count,
        }
    }

    /// 코드 커버리지 비율 (코드 라인 / 전체 라인)
    pub fn code_ratio(&self) -> f32 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.code_lines as f32 / self.total_lines as f32
        }
    }

    /// 주석 비율
    pub fn comment_ratio(&self) -> f32 {
        if self.total_lines == 0 {
            0.0
        } else {
            self.comment_lines as f32 / self.total_lines as f32
        }
    }

    /// 품질 점수 (0-100)
    pub fn quality_score(&self) -> u32 {
        let mut score = 100u32;

        // 함수 길이 패널티
        if self.max_function_length > 50 {
            score = score.saturating_sub(10);
        }
        if self.max_function_length > 100 {
            score = score.saturating_sub(10);
        }

        // 전역 변수 패널티
        if self.global_count > 5 {
            score = score.saturating_sub(5 * (self.global_count - 5).min(10) as u32);
        }

        // 주석 부족 패널티
        if self.code_lines > 50 && self.comment_ratio() < 0.1 {
            score = score.saturating_sub(10);
        }

        // TODO/FIXME 존재 패널티
        score = score.saturating_sub((self.todo_count + self.fixme_count).min(10) as u32);

        score
    }

    /// 품질 등급
    pub fn quality_grade(&self) -> &'static str {
        match self.quality_score() {
            90..=100 => "A",
            80..=89 => "B",
            70..=79 => "C",
            60..=69 => "D",
            _ => "F",
        }
    }
}

// ============================================================================
// Inline Issue Rendering
// ============================================================================

/// 인라인 이슈 렌더링 헬퍼
///
/// 코드 에디터에서 이슈를 인라인으로 표시하기 위한 데이터
pub struct InlineIssueRenderer;

impl InlineIssueRenderer {
    /// 라인에 대한 이슈 마커 생성
    ///
    /// 반환: (아이콘, 색상, 툴팁)
    pub fn get_line_marker(issues: &[&CodeIssue]) -> Option<(&'static str, [u8; 3], String)> {
        if issues.is_empty() {
            return None;
        }

        // 가장 심각한 이슈 선택
        let most_severe = issues.iter()
            .min_by_key(|i| match i.severity {
                IssueSeverity::Error => 0,
                IssueSeverity::Warning => 1,
                IssueSeverity::Info => 2,
                IssueSeverity::Hint => 3,
            })
            .unwrap();

        // 툴팁 생성
        let tooltip = if issues.len() == 1 {
            issues[0].message.clone()
        } else {
            format!("{} issues:\n{}", issues.len(),
                issues.iter()
                    .map(|i| format!("• [{}] {}", i.severity.display_name(), i.message))
                    .collect::<Vec<_>>()
                    .join("\n"))
        };

        Some((most_severe.severity.icon(), most_severe.severity.color(), tooltip))
    }

    /// 라인 하이라이트 색상 (배경색)
    pub fn get_line_highlight(issues: &[&CodeIssue]) -> Option<[u8; 4]> {
        if issues.is_empty() {
            return None;
        }

        // 가장 심각한 이슈의 색상 + 투명도
        let most_severe = issues.iter()
            .min_by_key(|i| match i.severity {
                IssueSeverity::Error => 0,
                IssueSeverity::Warning => 1,
                _ => 2,
            });

        most_severe.map(|i| {
            let [r, g, b] = i.severity.color();
            [r, g, b, 30] // 낮은 알파값으로 배경 하이라이트
        })
    }

    /// 인라인 힌트 텍스트 생성 (라인 끝에 표시)
    pub fn get_inline_hint(issues: &[&CodeIssue]) -> Option<String> {
        if issues.is_empty() {
            return None;
        }

        // 첫 번째 이슈만 표시 (공간 제약)
        let first = &issues[0];

        // 짧은 메시지로 축약
        let short_msg = if first.message.len() > 50 {
            format!("{}...", &first.message[..47])
        } else {
            first.message.clone()
        };

        Some(format!("  {} {}", first.severity.icon(), short_msg))
    }

    /// 스쿼글 언더라인 범위 계산
    pub fn get_squiggle_ranges(issues: &[&CodeIssue], line: usize) -> Vec<(usize, usize, [u8; 3])> {
        issues.iter()
            .filter(|i| i.range.start_line <= line && line <= i.range.end_line)
            .map(|i| {
                let start_col = if i.range.start_line == line {
                    i.range.start_column.saturating_sub(1)
                } else {
                    0
                };
                let end_col = if i.range.end_line == line {
                    i.range.end_column
                } else {
                    usize::MAX
                };
                (start_col, end_col, i.severity.color())
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_issue_creation() {
        let issue = CodeIssue::new(
            IssueSeverity::Warning,
            IssueCategory::Performance,
            PathBuf::from("test.lua"),
            10,
            "Avoid global variable access in loops",
        );

        assert_eq!(issue.severity, IssueSeverity::Warning);
        assert_eq!(issue.line(), 10);
        assert!(!issue.dismissed);
    }

    #[test]
    fn test_review_session() {
        let mut session = ReviewSession::new(PathBuf::from("test.lua"));
        assert_eq!(session.status, ReviewStatus::Idle);

        session.start_analysis();
        assert_eq!(session.status, ReviewStatus::Analyzing);

        let issues = vec![
            CodeIssue::new(IssueSeverity::Error, IssueCategory::Syntax, PathBuf::from("test.lua"), 5, "Syntax error"),
            CodeIssue::new(IssueSeverity::Warning, IssueCategory::Performance, PathBuf::from("test.lua"), 10, "Performance issue"),
        ];
        session.complete(issues);

        assert_eq!(session.status, ReviewStatus::Completed);
        assert_eq!(session.active_issue_count(), 2);
        assert_eq!(session.count_by_severity(IssueSeverity::Error), 1);
    }

    #[test]
    fn test_code_review_manager() {
        let mut manager = CodeReviewManager::new();

        let session = manager.request_review(PathBuf::from("test.lua"));
        assert_eq!(session.status, ReviewStatus::Analyzing);

        // 이슈 추가 시뮬레이션
        session.complete(vec![
            CodeIssue::new(IssueSeverity::Error, IssueCategory::Runtime, PathBuf::from("test.lua"), 1, "Error"),
        ]);

        assert_eq!(manager.total_error_count(), 1);
    }

    #[test]
    fn test_code_metrics_lua() {
        let lua_code = r#"
-- Test script
local function greet(name)
    print("Hello, " .. name)
end

-- TODO: Add more functions

function globalFunc()
    local x = 1
    return x
end

greet("World")
"#;

        let metrics = CodeMetrics::from_lua(lua_code);

        assert!(metrics.total_lines > 0);
        assert!(metrics.function_count >= 2);
        assert!(metrics.todo_count >= 1);
        assert!(metrics.comment_lines >= 2);
    }

    #[test]
    fn test_code_metrics_quality() {
        let mut metrics = CodeMetrics::new();
        metrics.code_lines = 100;
        metrics.total_lines = 120;
        metrics.comment_lines = 20;
        metrics.function_count = 5;
        metrics.max_function_length = 30;
        metrics.global_count = 2;

        let score = metrics.quality_score();
        assert!(score > 80, "Expected score > 80, got {}", score);
        assert_eq!(metrics.quality_grade(), "A");
    }

    #[test]
    fn test_inline_issue_renderer() {
        let issue1 = CodeIssue::new(
            IssueSeverity::Error,
            IssueCategory::Syntax,
            PathBuf::from("test.lua"),
            10,
            "Syntax error"
        );
        let issue2 = CodeIssue::new(
            IssueSeverity::Warning,
            IssueCategory::Performance,
            PathBuf::from("test.lua"),
            10,
            "Performance issue"
        );

        let issues: Vec<&CodeIssue> = vec![&issue1, &issue2];

        // 가장 심각한 이슈 (Error) 선택
        let marker = InlineIssueRenderer::get_line_marker(&issues);
        assert!(marker.is_some());
        let (icon, _color, tooltip) = marker.unwrap();
        assert_eq!(icon, "X"); // Error icon
        assert!(tooltip.contains("2 issues"));

        // 하이라이트
        let highlight = InlineIssueRenderer::get_line_highlight(&issues);
        assert!(highlight.is_some());
    }
}
