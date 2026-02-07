//! Rich Text — 리치 텍스트 마크업 파서 및 데코레이터
//!
//! 간단한 마크업 (<b>, <i>, <color>, <u>) 파싱으로
//! 스타일이 적용된 ITextRun 목록을 생성합니다.
//! UE5의 IRichTextMarkupParser, FDefaultRichTextMarkupParser에 해당합니다.

use crate::core::Color;
use super::text_run::{ITextRun, FSlateTextRun, TextRange, TextRunStyle};

// ============================================================================
// IRichTextMarkupParser trait
// ============================================================================

/// 리치 텍스트 마크업 파서 인터페이스 (UE5 IRichTextMarkupParser)
///
/// 마크업 문자열을 파싱하여 스타일이 적용된 텍스트 런 목록을 생성합니다.
pub trait IRichTextMarkupParser: Send + Sync {
    /// 마크업 텍스트를 파싱하여 런 목록 반환
    fn parse(&self, markup: &str, base_style: &TextRunStyle) -> Vec<Box<dyn ITextRun>>;
}

// ============================================================================
// ITextDecorator trait
// ============================================================================

/// 텍스트 데코레이터 인터페이스 (UE5 ITextDecorator)
///
/// 리치 텍스트 내 특정 태그에 대한 커스텀 스타일 적용.
pub trait ITextDecorator: Send + Sync {
    /// 이 데코레이터가 처리할 수 있는 태그 이름
    fn supports_tag(&self, tag: &str) -> bool;

    /// 태그에 따른 스타일 변환
    fn decorate(
        &self,
        tag: &str,
        attributes: &[(String, String)],
        base_style: &TextRunStyle,
    ) -> TextRunStyle;
}

// ============================================================================
// DefaultRichTextParser — 기본 마크업 파서
// ============================================================================

/// 기본 리치 텍스트 마크업 파서 (UE5 FDefaultRichTextMarkupParser)
///
/// 지원 태그:
/// - `<b>...</b>` — 볼드
/// - `<i>...</i>` — 이탤릭
/// - `<u>...</u>` — 밑줄
/// - `<s>...</s>` — 취소선
/// - `<color=#RRGGBB>...</color>` — 색상 변경
pub struct DefaultRichTextParser {
    /// 추가 데코레이터
    decorators: Vec<Box<dyn ITextDecorator>>,
}

impl DefaultRichTextParser {
    /// 새 파서 생성
    pub fn new() -> Self {
        Self {
            decorators: Vec::new(),
        }
    }

    /// 데코레이터 추가
    pub fn with_decorator(mut self, decorator: Box<dyn ITextDecorator>) -> Self {
        self.decorators.push(decorator);
        self
    }

    /// 태그 파싱 (간단한 <tag> 또는 <tag=value> 형식)
    fn parse_tag(tag_str: &str) -> Option<TagInfo> {
        let tag_str = tag_str.trim();
        if tag_str.is_empty() {
            return None;
        }

        let is_closing = tag_str.starts_with('/');
        let content = if is_closing { &tag_str[1..] } else { tag_str };

        // 태그 이름과 속성 분리
        let (name, attributes) = if let Some(eq_pos) = content.find('=') {
            let name = &content[..eq_pos];
            let value = content[eq_pos + 1..].trim_matches('"').trim_matches('\'');
            (name.to_lowercase(), vec![(String::new(), value.to_string())])
        } else if let Some(space_pos) = content.find(' ') {
            let name = &content[..space_pos];
            let rest = &content[space_pos + 1..];
            let attrs = Self::parse_attributes(rest);
            (name.to_lowercase(), attrs)
        } else {
            (content.to_lowercase(), Vec::new())
        };

        Some(TagInfo {
            name,
            is_closing,
            attributes,
        })
    }

    /// 속성 파싱 (key="value" 형식)
    fn parse_attributes(s: &str) -> Vec<(String, String)> {
        let mut attrs = Vec::new();
        let mut remaining = s.trim();

        while !remaining.is_empty() {
            if let Some(eq_pos) = remaining.find('=') {
                let key = remaining[..eq_pos].trim();
                let after_eq = &remaining[eq_pos + 1..];
                let (value, rest) = if after_eq.starts_with('"') {
                    if let Some(end_quote) = after_eq[1..].find('"') {
                        (&after_eq[1..=end_quote], &after_eq[end_quote + 2..])
                    } else {
                        (after_eq.trim_start_matches('"'), "")
                    }
                } else {
                    let end = after_eq.find(' ').unwrap_or(after_eq.len());
                    (&after_eq[..end], &after_eq[end..])
                };
                attrs.push((key.to_string(), value.to_string()));
                remaining = rest.trim();
            } else {
                break;
            }
        }

        attrs
    }

    /// 내장 태그에 따른 스타일 수정
    fn apply_builtin_tag(tag: &TagInfo, style: &TextRunStyle) -> TextRunStyle {
        let mut new_style = style.clone();

        match tag.name.as_str() {
            "b" | "bold" => {
                new_style.font_selector.weight = crate::core::FontWeight::Bold;
            }
            "i" | "italic" => {
                new_style.font_selector.style = crate::core::FontStyle::Italic;
            }
            "u" | "underline" => {
                new_style.underline = true;
            }
            "s" | "strikethrough" => {
                new_style.strikethrough = true;
            }
            "color" => {
                if let Some((_, value)) = tag.attributes.first() {
                    if let Some(color) = Color::from_hex(value) {
                        new_style.color = color;
                    }
                }
            }
            _ => {}
        }

        new_style
    }
}

impl Default for DefaultRichTextParser {
    fn default() -> Self {
        Self::new()
    }
}

impl IRichTextMarkupParser for DefaultRichTextParser {
    fn parse(&self, markup: &str, base_style: &TextRunStyle) -> Vec<Box<dyn ITextRun>> {
        let mut runs: Vec<Box<dyn ITextRun>> = Vec::new();
        let mut style_stack: Vec<TextRunStyle> = vec![base_style.clone()];
        let mut byte_offset: usize = 0;
        let mut current_text = String::new();

        let chars: Vec<char> = markup.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '<' {
                // 현재까지의 텍스트를 런으로 추가
                if !current_text.is_empty() {
                    let text_len = current_text.len();
                    let current_style = style_stack.last().unwrap_or(base_style).clone();
                    runs.push(Box::new(FSlateTextRun::new(
                        current_text.clone(),
                        current_style,
                        TextRange::new(byte_offset, byte_offset + text_len),
                    )));
                    byte_offset += text_len;
                    current_text.clear();
                }

                // 태그 찾기
                let tag_start = i + 1;
                if let Some(tag_end_offset) = chars[tag_start..].iter().position(|&c| c == '>') {
                    let tag_end = tag_start + tag_end_offset;
                    let tag_str: String = chars[tag_start..tag_end].iter().collect();

                    if let Some(tag_info) = Self::parse_tag(&tag_str) {
                        if tag_info.is_closing {
                            // 닫는 태그: 스타일 스택 팝
                            if style_stack.len() > 1 {
                                style_stack.pop();
                            }
                        } else {
                            // 여는 태그: 스타일 수정 후 스택 푸시
                            let current_style = style_stack.last().unwrap_or(base_style);

                            // 커스텀 데코레이터 체크
                            let mut new_style = None;
                            for decorator in &self.decorators {
                                if decorator.supports_tag(&tag_info.name) {
                                    new_style = Some(decorator.decorate(
                                        &tag_info.name,
                                        &tag_info.attributes,
                                        current_style,
                                    ));
                                    break;
                                }
                            }

                            let style = new_style
                                .unwrap_or_else(|| Self::apply_builtin_tag(&tag_info, current_style));
                            style_stack.push(style);
                        }
                    }

                    // 태그 바이트 건너뛰기 (< + content + >)
                    let tag_byte_len = chars[i..=tag_end].iter().map(|c| c.len_utf8()).sum::<usize>();
                    byte_offset += tag_byte_len;
                    i = tag_end + 1;
                } else {
                    // 닫히지 않은 '<' — 일반 문자로 처리
                    current_text.push(chars[i]);
                    i += 1;
                }
            } else {
                current_text.push(chars[i]);
                i += 1;
            }
        }

        // 남은 텍스트 처리
        if !current_text.is_empty() {
            let text_len = current_text.len();
            let current_style = style_stack.last().unwrap_or(base_style).clone();
            runs.push(Box::new(FSlateTextRun::new(
                current_text,
                current_style,
                TextRange::new(byte_offset, byte_offset + text_len),
            )));
        }

        runs
    }
}

// ============================================================================
// TagInfo (internal)
// ============================================================================

/// 파싱된 태그 정보
#[derive(Debug)]
struct TagInfo {
    name: String,
    is_closing: bool,
    attributes: Vec<(String, String)>,
}

// ============================================================================
// SyntaxTokenizer — 구문 강조 토크나이저
// ============================================================================

/// 구문 강조 토큰 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxTokenType {
    /// 일반 텍스트
    Normal,
    /// 키워드
    Keyword,
    /// 문자열 리터럴
    StringLiteral,
    /// 주석
    Comment,
    /// 숫자 리터럴
    Number,
    /// 연산자/구분자
    Operator,
    /// 타입/클래스 이름
    TypeName,
    /// 함수 이름
    Function,
}

/// 구문 강조 토큰
#[derive(Debug, Clone)]
pub struct SyntaxToken {
    /// 토큰 텍스트
    pub text: String,
    /// 토큰 타입
    pub token_type: SyntaxTokenType,
    /// 소스 내 바이트 범위
    pub range: TextRange,
}

/// 구문 강조 토크나이저 인터페이스
///
/// 소스 텍스트를 구문 토큰으로 분할합니다.
pub trait SyntaxTokenizer: Send + Sync {
    /// 소스 텍스트를 토큰 목록으로 분할
    fn tokenize(&self, source: &str) -> Vec<SyntaxToken>;

    /// 토큰 타입별 기본 색상 반환
    fn color_for_token(&self, token_type: SyntaxTokenType) -> Color {
        match token_type {
            SyntaxTokenType::Normal => Color::rgb(0.85, 0.85, 0.85),
            SyntaxTokenType::Keyword => Color::rgb(0.56, 0.69, 0.87),
            SyntaxTokenType::StringLiteral => Color::rgb(0.81, 0.54, 0.38),
            SyntaxTokenType::Comment => Color::rgb(0.42, 0.56, 0.42),
            SyntaxTokenType::Number => Color::rgb(0.71, 0.82, 0.55),
            SyntaxTokenType::Operator => Color::rgb(0.85, 0.85, 0.85),
            SyntaxTokenType::TypeName => Color::rgb(0.31, 0.78, 0.78),
            SyntaxTokenType::Function => Color::rgb(0.86, 0.86, 0.55),
        }
    }

    /// 토큰 목록을 스타일이 적용된 TextRun 목록으로 변환
    fn to_text_runs(&self, source: &str, base_style: &TextRunStyle) -> Vec<Box<dyn ITextRun>> {
        let tokens = self.tokenize(source);
        tokens
            .into_iter()
            .map(|token| {
                let color = self.color_for_token(token.token_type);
                let style = TextRunStyle {
                    color,
                    ..base_style.clone()
                };
                let run: Box<dyn ITextRun> = Box::new(FSlateTextRun::new(
                    token.text,
                    style,
                    token.range,
                ));
                run
            })
            .collect()
    }
}

// ============================================================================
// RichTextLayoutMarshaller — 리치 텍스트 마샬링
// ============================================================================

/// 리치 텍스트 레이아웃 마샬러 (UE5 FRichTextLayoutMarshaller)
///
/// IRichTextMarkupParser로 파싱된 텍스트 런을 TextLayout 엔진에 연결합니다.
/// 마크업 → 파싱 → 런 목록 → 레이아웃의 전체 파이프라인을 조율합니다.
pub struct RichTextLayoutMarshaller {
    /// 마크업 파서
    parser: Box<dyn IRichTextMarkupParser>,
    /// 기본 텍스트 스타일
    base_style: TextRunStyle,
    /// 캐시: 마지막 마크업 원본
    cached_markup: String,
    /// 캐시: 파싱된 런 목록
    cached_runs: Vec<Box<dyn ITextRun>>,
    /// 마크업 변경 여부
    is_dirty: bool,
}

impl RichTextLayoutMarshaller {
    /// 새 마샬러 생성 (기본 파서 사용)
    pub fn new() -> Self {
        Self {
            parser: Box::new(DefaultRichTextParser::new()),
            base_style: TextRunStyle::default(),
            cached_markup: String::new(),
            cached_runs: Vec::new(),
            is_dirty: true,
        }
    }

    /// 커스텀 파서로 생성
    pub fn with_parser(parser: Box<dyn IRichTextMarkupParser>) -> Self {
        Self {
            parser,
            base_style: TextRunStyle::default(),
            cached_markup: String::new(),
            cached_runs: Vec::new(),
            is_dirty: true,
        }
    }

    /// 기본 스타일 설정
    pub fn set_base_style(&mut self, style: TextRunStyle) {
        self.base_style = style;
        self.is_dirty = true;
    }

    /// 기본 스타일 참조
    pub fn base_style(&self) -> &TextRunStyle {
        &self.base_style
    }

    /// 파서 교체
    pub fn set_parser(&mut self, parser: Box<dyn IRichTextMarkupParser>) {
        self.parser = parser;
        self.is_dirty = true;
    }

    /// 마크업 텍스트 설정
    pub fn set_text(&mut self, markup: &str) {
        if self.cached_markup != markup {
            self.cached_markup = markup.to_string();
            self.is_dirty = true;
        }
    }

    /// 현재 마크업 텍스트
    pub fn text(&self) -> &str {
        &self.cached_markup
    }

    /// 파싱된 런 목록 가져오기 (캐시 활용)
    pub fn get_runs(&mut self) -> &[Box<dyn ITextRun>] {
        if self.is_dirty {
            self.cached_runs = self.parser.parse(&self.cached_markup, &self.base_style);
            self.is_dirty = false;
        }
        &self.cached_runs
    }

    /// 런 개수
    pub fn run_count(&mut self) -> usize {
        self.get_runs().len()
    }

    /// 일반 텍스트 추출 (마크업 제거)
    pub fn plain_text(&mut self) -> String {
        let runs = self.get_runs();
        runs.iter().map(|r| r.text().to_string()).collect::<Vec<_>>().join("")
    }

    /// 레이아웃 수행 (TextLayout과 연동)
    ///
    /// 내부적으로 파싱 → 런 생성 → TextLayout::layout 호출
    pub fn layout(
        &mut self,
        params: &super::text_layout::TextLayoutParams,
    ) -> super::text_layout::TextLayoutResult {
        let runs = self.get_runs();
        super::text_layout::TextLayout::layout(runs, params)
    }

    /// 강제 무효화 (다음 get_runs 시 재파싱)
    pub fn invalidate(&mut self) {
        self.is_dirty = true;
    }

    /// 캐시가 유효한지 여부
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }
}

impl Default for RichTextLayoutMarshaller {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::FontWeight;

    #[test]
    fn test_plain_text_parse() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("Hello World", &style);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text(), "Hello World");
    }

    #[test]
    fn test_bold_tag_parse() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("Normal <b>Bold</b> Normal", &style);
        assert_eq!(runs.len(), 3);
        assert_eq!(runs[0].text(), "Normal ");
        assert_eq!(runs[1].text(), "Bold");
        assert_eq!(runs[1].style().font_selector.weight, FontWeight::Bold);
        assert_eq!(runs[2].text(), " Normal");
    }

    #[test]
    fn test_nested_tags() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("<b><i>BoldItalic</i></b>", &style);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text(), "BoldItalic");
        assert_eq!(runs[0].style().font_selector.weight, FontWeight::Bold);
        assert_eq!(runs[0].style().font_selector.style, crate::core::FontStyle::Italic);
    }

    #[test]
    fn test_color_tag() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("<color=#FF0000>Red</color>", &style);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text(), "Red");
        assert_eq!(runs[0].style().color.r, 1.0);
        assert_eq!(runs[0].style().color.g, 0.0);
    }

    #[test]
    fn test_underline_tag() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("<u>Underlined</u>", &style);
        assert_eq!(runs.len(), 1);
        assert!(runs[0].style().underline);
    }

    #[test]
    fn test_empty_markup() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("", &style);
        assert!(runs.is_empty());
    }

    #[test]
    fn test_unclosed_tag_as_text() {
        let parser = DefaultRichTextParser::new();
        let style = TextRunStyle::default();
        let runs = parser.parse("Hello <unclosed", &style);
        // '<' 뒤에 '>'를 찾지 못하면 '<'은 일반 문자로 처리
        assert!(!runs.is_empty());
    }

    #[test]
    fn test_syntax_token_type() {
        assert_eq!(SyntaxTokenType::Normal, SyntaxTokenType::Normal);
        assert_ne!(SyntaxTokenType::Keyword, SyntaxTokenType::Comment);
    }

    // --- RichTextLayoutMarshaller tests ---

    #[test]
    fn test_marshaller_basic() {
        let mut m = RichTextLayoutMarshaller::new();
        m.set_text("Hello <b>World</b>");
        assert_eq!(m.run_count(), 2);
        assert_eq!(m.plain_text(), "Hello World");
    }

    #[test]
    fn test_marshaller_caching() {
        let mut m = RichTextLayoutMarshaller::new();
        m.set_text("Test");
        let _ = m.get_runs();
        assert!(!m.is_dirty());

        // 같은 텍스트 → dirty 아님
        m.set_text("Test");
        assert!(!m.is_dirty());

        // 다른 텍스트 → dirty
        m.set_text("Changed");
        assert!(m.is_dirty());
    }

    #[test]
    fn test_marshaller_invalidate() {
        let mut m = RichTextLayoutMarshaller::new();
        m.set_text("Hello");
        let _ = m.get_runs();
        assert!(!m.is_dirty());

        m.invalidate();
        assert!(m.is_dirty());
    }

    #[test]
    fn test_marshaller_with_style() {
        let mut m = RichTextLayoutMarshaller::new();
        let style = TextRunStyle {
            font_size: 20.0,
            color: Color::rgb(1.0, 0.0, 0.0),
            ..Default::default()
        };
        m.set_base_style(style);
        m.set_text("Styled");

        let runs = m.get_runs();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].style().font_size, 20.0);
    }

    #[test]
    fn test_marshaller_default() {
        let m = RichTextLayoutMarshaller::default();
        assert!(m.is_dirty());
        assert_eq!(m.text(), "");
    }
}
