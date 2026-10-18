//! Error definitions for the dubbing audio mixing domain.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DubbingMixError {
    #[error("Dubbing project '{0}' not found")]
    ProjectNotFound(String),

    #[error(
        "No audio inputs available to mixdown (no original audio and no active dubbing segments)"
    )]
    NoAudioInputs,

    #[error("Original audio file '{0}' was not found or is inaccessible")]
    OriginalAudioNotFound(String),

    #[error("Dubbing audio asset '{0}' was not found on disk")]
    AssetNotFound(String),

    #[error("Audio asset '{0}' failed validation: {1}")]
    AssetCorrupted(String, String),

    #[error("Invalid gain value '{0}' dB: {1}")]
    InvalidGain(f32, String),

    #[error("FFmpeg mixdown execution failed: {0}")]
    FFmpegExecutionFailed(String),

    #[error("Audio analysis failed: {0}")]
    AnalysisFailed(String),

    #[error("Mixdown rendering was cancelled by user")]
    RenderCancelled,

    #[error("Filesystem IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("WAV header parsing error: {0}")]
    WavParseError(String),
}
