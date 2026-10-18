//! Error types for Dubbing Duration Alignment.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DubbingAlignmentError {
    #[error("Segment '{0}' not found in project")]
    SegmentNotFound(String),

    #[error("Audio asset '{0}' not found in project manifest")]
    AssetNotFound(String),

    #[error("Source audio file not found on disk: {0}")]
    AudioFileNotFound(String),

    #[error("Invalid source audio trim interval: in={0}us, out={1}us, total={2}us")]
    InvalidTrimInterval(u64, u64, u64),

    #[error("Target duration must be greater than zero")]
    ZeroTargetDuration,

    #[error("Requested tempo factor {0:.2} is outside permitted range [{1:.2}, {2:.2}]")]
    TempoOutOfRange(f32, f32, f32),

    #[error("FFmpeg tool not found or failed execution: {0}")]
    FfmpegExecutionFailed(String),

    #[error("Output WAV validation failed: {0}")]
    InvalidOutputWav(String),

    #[error("Alignment processing was cancelled")]
    Cancelled,

    #[error("Failed to read or write audio file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Audio processing failed: {0}")]
    ProcessingFailed(String),
}
