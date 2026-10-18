//! Deterministic planner for AI Dubbing Duration Alignment.

use crate::localization::dubbing::models::{DubbingAudioAsset, DubbingSegment};

use super::errors::DubbingAlignmentError;
use super::models::{AlignmentConfig, DubbingAlignmentPlan, SilencePaddingMode};

/// Computes an explainable duration alignment plan for a dubbing segment.
pub fn plan_segment_alignment(
    segment: &DubbingSegment,
    asset: &DubbingAudioAsset,
    config: &AlignmentConfig,
    manual_tempo: Option<f32>,
    manual_padding: Option<SilencePaddingMode>,
    manual_offset_us: Option<i64>,
    next_segment_start_us: Option<u64>,
) -> Result<DubbingAlignmentPlan, DubbingAlignmentError> {
    // 1. Validate trim boundaries
    if segment.source_in_us >= segment.source_out_us {
        return Err(DubbingAlignmentError::InvalidTrimInterval(
            segment.source_in_us,
            segment.source_out_us,
            asset.duration_us,
        ));
    }
    let trimmed_us = segment.trimmed_duration_us();
    if trimmed_us == 0 {
        return Err(DubbingAlignmentError::InvalidTrimInterval(
            segment.source_in_us,
            segment.source_out_us,
            asset.duration_us,
        ));
    }

    // 2. Resolve target cue interval
    let target_start_us = segment
        .original_cue_start_us
        .unwrap_or(segment.timeline_start_us);
    let target_end_us = segment
        .original_cue_end_us
        .unwrap_or(target_start_us.saturating_add(trimmed_us));
    let target_duration_us = target_end_us.saturating_sub(target_start_us);

    if target_duration_us == 0 {
        return Err(DubbingAlignmentError::ZeroTargetDuration);
    }

    // 3. Resolve start offset
    let requested_start_offset_us =
        manual_offset_us.unwrap_or_else(|| segment.start_offset_us.unwrap_or(0));

    // 4. Calculate effective target duration available for speech
    let effective_target_duration_us = if requested_start_offset_us >= 0 {
        target_duration_us.saturating_sub(requested_start_offset_us as u64)
    } else {
        target_duration_us.saturating_add((-requested_start_offset_us) as u64)
    };

    let mut warnings = Vec::new();

    // Check if offset creates boundary or collision issues
    if requested_start_offset_us < 0 && (target_start_us as i64 + requested_start_offset_us) < 0 {
        warnings
            .push("Negative start offset places audio before timeline start (0.0s)".to_string());
    }

    // 5. Build alignment strategy
    let (
        proposed_tempo,
        padding_mode,
        padding_before_us,
        padding_after_us,
        selected_strategy,
        requires_manual_review,
    ) = if let Some(user_tempo) = manual_tempo {
        // Manual tempo override mode
        if user_tempo < config.min_supported_tempo || user_tempo > config.max_supported_tempo {
            return Err(DubbingAlignmentError::TempoOutOfRange(
                user_tempo,
                config.min_supported_tempo,
                config.max_supported_tempo,
            ));
        }

        let tempo_duration_us = ((trimmed_us as f64) / (user_tempo as f64)).round() as u64;
        let pad_mode = manual_padding.unwrap_or(SilencePaddingMode::NoPadding);
        let (before_us, after_us) = if tempo_duration_us < effective_target_duration_us {
            let shortfall = effective_target_duration_us - tempo_duration_us;
            calculate_padding(shortfall, pad_mode)
        } else {
            (0, 0)
        };

        let strategy = format!(
            "Manual tempo override ({:.2}x){}",
            user_tempo,
            if before_us > 0 || after_us > 0 {
                format!(
                    " with silence padding (+{:.2}s)",
                    (before_us + after_us) as f64 / 1_000_000.0
                )
            } else {
                String::new()
            }
        );

        (user_tempo, pad_mode, before_us, after_us, strategy, false)
    } else {
        // AutoFit mode
        let diff = (trimmed_us as i64) - (effective_target_duration_us as i64);

        if diff.unsigned_abs() <= config.tolerance_us {
            // Within tolerance: keep original tempo
            (
                    1.0,
                    SilencePaddingMode::NoPadding,
                    0,
                    0,
                    "Original tempo preserved (speech duration matches subtitle interval within tolerance)".to_string(),
                    false,
                )
        } else if diff < 0 {
            // Speech is shorter than target: pad with silence
            let shortfall_us = (-diff) as u64;
            let pad_mode = manual_padding.unwrap_or(config.default_padding_mode);
            let (before_us, after_us) = calculate_padding(shortfall_us, pad_mode);

            let strategy = format!(
                "Speech shorter than subtitle by {:.2}s; padded with silence",
                shortfall_us as f64 / 1_000_000.0
            );

            (1.0, pad_mode, before_us, after_us, strategy, false)
        } else {
            // Speech is longer than target: consider tempo acceleration
            let ideal_tempo = (trimmed_us as f64) / (effective_target_duration_us as f64);
            let rounded_tempo = ((ideal_tempo * 100.0).round() / 100.0) as f32;

            if ideal_tempo <= config.max_auto_tempo as f64 {
                (
                    rounded_tempo,
                    SilencePaddingMode::NoPadding,
                    0,
                    0,
                    format!(
                        "Pitch-preserving acceleration ({:.2}x tempo) to fit subtitle window",
                        rounded_tempo
                    ),
                    false,
                )
            } else if ideal_tempo <= config.max_supported_tempo as f64 {
                warnings.push(format!(
                    "Required speedup ({:.2}x) exceeds conservative AutoFit limit ({:.2}x)",
                    rounded_tempo, config.max_auto_tempo
                ));
                (
                        rounded_tempo,
                        SilencePaddingMode::NoPadding,
                        0,
                        0,
                        format!("Required speedup ({:.2}x) exceeds auto limit ({:.2}x); manual review recommended", rounded_tempo, config.max_auto_tempo),
                        true,
                    )
            } else {
                warnings.push(format!(
                        "Speech duration ({:.2}s) significantly exceeds cue interval ({:.2}s); extreme tempo ({:.2}x) required",
                        trimmed_us as f64 / 1_000_000.0,
                        effective_target_duration_us as f64 / 1_000_000.0,
                        ideal_tempo
                    ));
                (
                    1.0,
                    SilencePaddingMode::NoPadding,
                    0,
                    0,
                    "Speech duration significantly exceeds cue interval; automatic fit infeasible"
                        .to_string(),
                    true,
                )
            }
        }
    };

    // 6. Check timeline collision with next segment
    if let Some(next_start) = next_segment_start_us {
        let expected_end_us = target_start_us
            .saturating_add(requested_start_offset_us.max(0) as u64)
            .saturating_add(
                ((trimmed_us as f64 / proposed_tempo as f64).round() as u64)
                    + padding_before_us
                    + padding_after_us,
            );

        if expected_end_us > next_start {
            let collision_us = expected_end_us - next_start;
            warnings.push(format!(
                "Aligned audio will overlap with following segment by {:.2}s",
                collision_us as f64 / 1_000_000.0
            ));
        }
    }

    Ok(DubbingAlignmentPlan {
        segment_id: segment.id.clone(),
        original_asset_id: asset.asset_id.clone(),
        original_asset_hash: asset.sha256.clone(),
        source_trimmed_duration_us: trimmed_us,
        target_start_us,
        target_end_us,
        target_duration_us,
        requested_start_offset_us,
        effective_target_duration_us,
        proposed_tempo,
        padding_mode,
        padding_before_us,
        padding_after_us,
        selected_strategy,
        requires_manual_review,
        warnings,
        algorithm_version: config.algorithm_version,
    })
}

/// Helper calculating before and after silence padding based on padding mode.
fn calculate_padding(shortfall_us: u64, mode: SilencePaddingMode) -> (u64, u64) {
    match mode {
        SilencePaddingMode::NoPadding => (0, 0),
        SilencePaddingMode::PadEnd => (0, shortfall_us),
        SilencePaddingMode::PadStart => (shortfall_us, 0),
        SilencePaddingMode::Centered => {
            let before = shortfall_us / 2;
            let after = shortfall_us - before;
            (before, after)
        }
    }
}
