//! 로딩 진행 상황 및 초기화 단계 정의

#![allow(dead_code)]

/// 초기화 단계
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitStage {
    /// GPU 기본 초기화 완료, 렌더러 생성 시작
    Renderers,
    /// 텍스처 및 머티리얼 로딩
    Textures,
    /// 메시 버퍼 및 ECS 리소스
    Meshes,
    /// 씬 파일 로드
    Scene,
    /// 캐릭터 모델 로드
    Characters,
    /// 오디오, Prefab 등 최종화
    Finalize,
    /// 초기화 완료
    Complete,
}

impl InitStage {
    /// 단계 인덱스 (0-6)
    pub fn index(&self) -> u32 {
        match self {
            InitStage::Renderers => 0,
            InitStage::Textures => 1,
            InitStage::Meshes => 2,
            InitStage::Scene => 3,
            InitStage::Characters => 4,
            InitStage::Finalize => 5,
            InitStage::Complete => 6,
        }
    }

    /// 현재 단계의 진행률 (0.0 ~ 1.0)
    pub fn progress(&self) -> f32 {
        match self {
            InitStage::Renderers => 0.10,
            InitStage::Textures => 0.30,
            InitStage::Meshes => 0.50,
            InitStage::Scene => 0.70,
            InitStage::Characters => 0.85,
            InitStage::Finalize => 0.95,
            InitStage::Complete => 1.0,
        }
    }

    /// 현재 단계의 표시 텍스트
    pub fn display_text(&self) -> &'static str {
        match self {
            InitStage::Renderers => "Creating renderers...",
            InitStage::Textures => "Loading textures...",
            InitStage::Meshes => "Loading meshes...",
            InitStage::Scene => "Loading scene...",
            InitStage::Characters => "Loading characters...",
            InitStage::Finalize => "Finalizing...",
            InitStage::Complete => "Ready!",
        }
    }

    /// 다음 단계로 진행
    pub fn next(&self) -> Option<InitStage> {
        match self {
            InitStage::Renderers => Some(InitStage::Textures),
            InitStage::Textures => Some(InitStage::Meshes),
            InitStage::Meshes => Some(InitStage::Scene),
            InitStage::Scene => Some(InitStage::Characters),
            InitStage::Characters => Some(InitStage::Finalize),
            InitStage::Finalize => Some(InitStage::Complete),
            InitStage::Complete => None,
        }
    }
}

/// 로딩 진행 상황
#[derive(Debug, Clone)]
pub struct LoadingProgress {
    pub stage: InitStage,
    pub sub_progress: f32,  // 현재 단계 내 세부 진행률 (0.0 ~ 1.0)
}

impl Default for LoadingProgress {
    fn default() -> Self {
        Self {
            stage: InitStage::Renderers,
            sub_progress: 0.0,
        }
    }
}

impl LoadingProgress {
    /// 전체 진행률 계산 (0.0 ~ 1.0)
    pub fn total_progress(&self) -> f32 {
        let base = match self.stage {
            InitStage::Renderers => 0.0,
            InitStage::Textures => 0.10,
            InitStage::Meshes => 0.30,
            InitStage::Scene => 0.50,
            InitStage::Characters => 0.70,
            InitStage::Finalize => 0.85,
            InitStage::Complete => 1.0,
        };

        let range = match self.stage {
            InitStage::Renderers => 0.10,
            InitStage::Textures => 0.20,
            InitStage::Meshes => 0.20,
            InitStage::Scene => 0.20,
            InitStage::Characters => 0.15,
            InitStage::Finalize => 0.10,
            InitStage::Complete => 0.0,
        };

        base + range * self.sub_progress
    }
}

/// 초기화 컨텍스트 - 단계별 초기화를 위한 상태 저장
pub struct InitContext {
    pub stage: InitStage,
    pub progress: LoadingProgress,
}

impl Default for InitContext {
    fn default() -> Self {
        Self {
            stage: InitStage::Renderers,
            progress: LoadingProgress::default(),
        }
    }
}

impl InitContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// 현재 단계 텍스트
    pub fn stage_text(&self) -> &'static str {
        self.stage.display_text()
    }

    /// 전체 진행률
    pub fn total_progress(&self) -> f32 {
        self.progress.total_progress()
    }

    /// 다음 단계로 진행
    pub fn advance(&mut self) -> bool {
        if let Some(next) = self.stage.next() {
            self.stage = next;
            self.progress.stage = next;
            self.progress.sub_progress = 0.0;
            true
        } else {
            false
        }
    }

    /// 초기화 완료 여부
    pub fn is_complete(&self) -> bool {
        self.stage == InitStage::Complete
    }
}
