//! Dubbing Duration Alignment module.
//!
//! Provides pitch-preserving tempo adjustment, silence padding, synchronization
//! optimization, explainable AutoFit planning, and non-destructive audio caching.

pub mod diagnostics;
pub mod errors;
pub mod models;
pub mod padding;
pub mod planner;
pub mod processor;
pub mod tempo;

pub use diagnostics::evaluate_alignment_diagnostics;
pub use errors::DubbingAlignmentError;
pub use models::{
    AlignmentConfig, AlignmentSeverity, DubbingAlignmentDiagnosticCode, DubbingAlignmentFinding,
    DubbingAlignmentMode, DubbingAlignmentPlan, DubbingAlignmentResult, DubbingAlignmentStatus,
    SilencePaddingMode,
};
pub use padding::pad_audio_with_silence;
pub use planner::plan_segment_alignment;
pub use processor::{compute_alignment_cache_key, execute_alignment_plan};
pub use tempo::{build_atempo_filter, build_tempo_process_args, process_audio_tempo};
