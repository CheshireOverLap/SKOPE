//! Audio Error Types

// ============ Error Types ============

#[derive(Debug)]
pub enum AudioError {
    InitFailed(String),
    SoundNotFound(String),
    PlayFailed(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::InitFailed(e) => write!(f, "Audio init failed: {}", e),
            AudioError::SoundNotFound(name) => write!(f, "Sound not found: {}", name),
            AudioError::PlayFailed(e) => write!(f, "Play failed: {}", e),
        }
    }
}

impl std::error::Error for AudioError {}
