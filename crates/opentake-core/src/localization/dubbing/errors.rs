//! Error types for the AI Dubbing Timeline module.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DubbingError {
    #[error("Dubbing track not found: {0}")]
    TrackNotFound(String),

    #[error("Dubbing segment not found: {0}")]
    SegmentNotFound(String),

    #[error("Dubbing audio asset not found: {0}")]
    AssetNotFound(String),

    #[error("Subtitle cue not found: {0}")]
    CueNotFound(String),

    #[error("Invalid time interval: start ({start_us} µs) >= end ({end_us} µs)")]
    InvalidInterval { start_us: u64, end_us: u64 },

    #[error("Invalid trim boundaries: source_in ({source_in_us} µs) >= source_out ({source_out_us} µs) or exceeds asset duration ({duration_us} µs)")]
    InvalidTrim {
        source_in_us: u64,
        source_out_us: u64,
        duration_us: u64,
    },

    #[error("Invalid split position: split time ({split_timeline_us} µs) falls outside segment bounds [{start_us} µs, {end_us} µs]")]
    InvalidSplit {
        split_timeline_us: u64,
        start_us: u64,
        end_us: u64,
    },

    #[error("Incompatible track language: track language '{track_lang}' does not match segment language '{segment_lang}'")]
    IncompatibleLanguage {
        track_lang: String,
        segment_lang: String,
    },

    #[error("Voice model not assigned or unavailable for track '{0}'")]
    VoiceUnavailable(String),

    #[error("Audio validation failure: {0}")]
    AudioValidationFailed(String),

    #[error("TTS synthesis error: {0}")]
    TtsError(String),

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
