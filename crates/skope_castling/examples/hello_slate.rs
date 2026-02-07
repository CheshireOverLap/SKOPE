//! Hello Slate - skope_castling 예제
//!
//! 실행: cargo run -p skope_castling --example hello_slate --features app

use skope_castling::prelude::*;

/// 앱 상태
struct HelloApp {
    root: SVerticalBox,
}

impl HelloApp {
    fn new() -> Self {
        // UI 구성 (빌더 패턴)
        let root = SVerticalBox::new()
            // 제목
            .slot()
                .padding(Margin::new(20.0, 40.0, 20.0, 10.0))
                .h_align(HAlign::Center)
                .auto_height()
                .content(
                    STextBlock::new()
                        .text("Hello, Slate UI!")
                        .font_size(28.0)
                        .color(Color::rgba(1.0, 1.0, 1.0, 1.0))
                        .build()
                )
            // 설명
            .slot()
                .padding(Margin::symmetric(20.0, 5.0))
                .h_align(HAlign::Center)
                .auto_height()
                .content(
                    STextBlock::new()
                        .text("Rust로 구현한 Slate 스타일 UI")
                        .font_size(16.0)
                        .color(Color::rgba(0.7, 0.7, 0.7, 1.0))
                        .build()
                )
            // 버튼
            .slot()
                .padding(Margin::uniform(30.0))
                .h_align(HAlign::Center)
                .auto_height()
                .content(
                    SBox::new()
                        .width(200.0)
                        .height(50.0)
                        .content(
                            SButton::new()
                                .background_color(Color::rgba(0.2, 0.4, 0.8, 1.0))
                                .content(
                                    STextBlock::new()
                                        .text("Click Me!")
                                        .font_size(18.0)
                                        .color(Color::rgba(1.0, 1.0, 1.0, 1.0))
                                        .build()
                                )
                                .on_clicked(|| {
                                    log::info!("Button clicked!");
                                    Reply::handled()
                                })
                                .build()
                        )
                        .build()
                )
            // 하단 정보
            .slot()
                .padding(Margin::uniform(10.0))
                .h_align(HAlign::Center)
                .auto_height()
                .content(
                    STextBlock::new()
                        .text("skope_castling v0.1.0")
                        .font_size(12.0)
                        .color(Color::rgba(0.5, 0.5, 0.5, 1.0))
                        .build()
                )
            .build();

        Self { root }
    }
}

impl SlateAppHandler for HelloApp {
    fn root_widget(&mut self) -> &mut dyn Widget {
        &mut self.root
    }

    fn update(&mut self, _delta_time: f32) {
        // 프레임 업데이트 로직
    }

    fn on_resize(&mut self, width: u32, height: u32) {
        log::info!("Window resized: {}x{}", width, height);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 로거 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    // 폰트 로드
    let font_path = "assets/fonts/NotoSansCJK-Regular.ttc";
    let font_data = std::fs::read(font_path).unwrap_or_else(|_| {
        log::warn!("Font not found at {}, trying system font...", font_path);
        // Windows 시스템 폰트 시도
        std::fs::read("C:/Windows/Fonts/segoeui.ttf")
            .unwrap_or_else(|_| {
                log::error!("No font available!");
                Vec::new()
            })
    });

    // 앱 설정
    let config = SlateAppConfig::new("Hello Slate")
        .with_size(800, 600)
        .with_clear_color(0.1, 0.1, 0.12, 1.0)
        .with_font(font_data);

    // 앱 생성 및 실행
    let app = SlateApp::new(config, HelloApp::new());
    app.run()
}
