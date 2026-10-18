//! Canonical data contracts and structures for AI Dubbing Duration Alignment.

use serde::{Deserialize, Serialize};

/// Alignment strategy mode for a dubbing segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DubbingAlignmentMode {
    /// Original generated speech audio without speed or padding modifications.
    #[default]
    Original,
    /// Automatic conservative alignment (pitch-preserving tempo adjustment and/or silence padding).
    AutoFit,
    /// Explicit tempo adjustment.
    TempoAdjust,
    /// Explicit silence padding.
    PadSilence,
    /// User-customized tempo, padding, and start offset.
    Manual,
}

/// Operational status of an alignment analysis or processing task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DubbingAlignmentStatus {
    /// Segment timing has not been analyzed against subtitle cue interval.
    #[default]
    NotAnalyzed,
    /// Analysis complete; alignment plan is ready for review or processing.
    Ready,
    /// Background FFmpeg tempo/padding processing is actively executing.
    Processing,
    /// Processed aligned audio asset successfully generated and applied.
    Aligned,
    /// Timing mismatch is too large for automatic alignment; user manual review required.
    NeedsReview,
    /// Alignment processing failed.
    Failed,
    /// Alignment job was cancelled by user.
    Cancelled,
}

/// Silence padding placement mode when speech is shorter than target interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SilencePaddingMode {
    /// No silence padding applied.
    NoPadding,
    /// Silence added at the end (preserves spoken start timing).
    #[default]
    PadEnd,
    /// Silence added at the start (delays spoken start timing).
    PadStart,
    /// Silence divided evenly before and after dialogue.
    Centered,
}

/// Diagnostic finding codes specific to duration alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DubbingAlignmentDiagnosticCode {
    SpeechLongerThanCue,
    SpeechShorterThanCue,
    TempoExceedsAutoLimit,
    TempoBelowAutoLimit,
    AlignmentRequiresReview,
    AlignedDurationMismatch,
    DialogueOverlap,
    OutsideVideoBounds,
    MissingSourceAudio,
    MissingAlignedAudio,
    CorruptedAlignedAudio,
    StaleAlignment,
    SourceTrimChanged,
    VoiceSegmentRegenerated,
    ManualOffsetConflict,
    InsufficientTimelineSpace,
}

/// Severity level for alignment diagnostic findings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlignmentSeverity {
    Info,
    Warning,
    Error,
}

/// A structured finding reported during alignment planning or validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingAlignmentFinding {
    pub code: DubbingAlignmentDiagnosticCode,
    pub severity: AlignmentSeverity,
    pub segment_id: String,
    pub message: String,
    pub duration_diff_us: Option<i64>,
}

/// Configuration policies for automatic alignment planning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentConfig {
    /// Conservative minimum tempo factor for AutoFit (default: 0.85).
    pub min_auto_tempo: f32,
    /// Conservative maximum tempo factor for AutoFit (default: 1.15).
    pub max_auto_tempo: f32,
    /// Absolute minimum supported tempo factor (default: 0.50).
    pub min_supported_tempo: f32,
    /// Absolute maximum supported tempo factor (default: 2.00).
    pub max_supported_tempo: f32,
    /// Timing tolerance in microseconds below which tempo modification is skipped (default: 25,000 µs = 25ms).
    pub tolerance_us: u64,
    /// Default silence padding strategy when speech is shorter than target.
    pub default_padding_mode: SilencePaddingMode,
    /// Algorithm configuration version.
    pub algorithm_version: u32,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            min_auto_tempo: 0.85,
            max_auto_tempo: 1.15,
            min_supported_tempo: 0.50,
            max_supported_tempo: 2.00,
            tolerance_us: 25_000,
            default_padding_mode: SilencePaddingMode::PadEnd,
            algorithm_version: 1,
        }
    }
}

/// An explainable duration alignment plan for a specific dubbing segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingAlignmentPlan {
    /// Segment UUID.
    pub segment_id: String,
    /// Original audio asset UUID.
    pub original_asset_id: String,
    /// SHA-256 digest of original audio asset.
    pub original_asset_hash: String,
    /// Source trimmed audio duration in microseconds.
    pub source_trimmed_duration_us: u64,
    /// Subtitle cue interval start in microseconds.
    pub target_start_us: u64,
    /// Subtitle cue interval end in microseconds.
    pub target_end_us: u64,
    /// Available subtitle window duration in microseconds (`target_end_us - target_start_us`).
    pub target_duration_us: u64,
    /// Manual start offset in microseconds relative to cue start.
    pub requested_start_offset_us: i64,
    /// Effective target duration after offset adjustments.
    pub effective_target_duration_us: u64,
    /// Proposed pitch-preserving tempo multiplier (1.0 = normal speed).
    pub proposed_tempo: f32,
    /// Chosen silence padding mode.
    pub padding_mode: SilencePaddingMode,
    /// Microseconds of silence to prepend before dialogue.
    pub padding_before_us: u64,
    /// Microseconds of silence to append after dialogue.
    pub padding_after_us: u64,
    /// Human-readable explanation of chosen strategy.
    pub selected_strategy: String,
    /// Flag indicating whether the plan exceeds safe limits and requires manual user approval.
    pub requires_manual_review: bool,
    /// Non-fatal diagnostic warning messages.
    pub warnings: Vec<String>,
    /// Algorithm version used to generate this plan.
    pub algorithm_version: u32,
}

/// Concrete result of executing an alignment plan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingAlignmentResult {
    /// Stable alignment execution UUID.
    pub alignment_id: String,
    /// Associated segment UUID.
    pub segment_id: String,
    /// Original input audio asset UUID.
    pub original_asset_id: String,
    /// Generated aligned audio asset UUID (if processed).
    pub aligned_asset_id: Option<String>,
    /// Measured output audio duration in microseconds from decoded samples.
    pub actual_output_duration_us: u64,
    /// Effective timeline start in microseconds.
    pub effective_placement_start_us: u64,
    /// Effective timeline end in microseconds (`start + actual_output_duration`).
    pub effective_placement_end_us: u64,
    /// Description of processing applied (e.g. "atempo=1.12 + pad_end=150ms").
    pub processing_method: String,
    /// Signed duration error relative to target (`actual_output_duration_us - target_duration_us`).
    pub measured_duration_error_us: i64,
    /// Exact tempo factor applied by audio filter.
    pub tempo_factor_applied: f32,
    /// Exact padding microseconds applied before dialogue.
    pub padding_before_applied_us: u64,
    /// Exact padding microseconds applied after dialogue.
    pub padding_after_applied_us: u64,
    /// Post-processing diagnostics.
    pub diagnostics: Vec<DubbingAlignmentFinding>,
    /// Alignment outcome status.
    pub status: DubbingAlignmentStatus,
    /// Execution timestamp (Unix epoch milliseconds).
    pub created_at_epoch_ms: u64,
}
