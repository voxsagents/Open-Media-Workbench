//! Diagnostic analysis for Dubbing Duration Alignment.

use crate::localization::dubbing::models::DubbingProject;

use super::models::{
    AlignmentConfig, AlignmentSeverity, DubbingAlignmentDiagnosticCode, DubbingAlignmentFinding,
};

/// Evaluates duration alignment diagnostics across all segments in a dubbing project.
pub fn evaluate_alignment_diagnostics(
    project: &DubbingProject,
    config: &AlignmentConfig,
) -> Vec<DubbingAlignmentFinding> {
    let mut findings = Vec::new();

    for track in &project.tracks {
        let mut prev_segment_end: Option<u64> = None;

        for segment in &track.segments {
            let trimmed_us = segment.trimmed_duration_us();
            let target_start_us = segment
                .original_cue_start_us
                .unwrap_or(segment.timeline_start_us);
            let target_end_us = segment
                .original_cue_end_us
                .unwrap_or(target_start_us.saturating_add(trimmed_us));
            let target_duration_us = target_end_us.saturating_sub(target_start_us);

            let diff_us = (trimmed_us as i64) - (target_duration_us as i64);

            // 1. Duration mismatch findings
            if diff_us > (config.tolerance_us as i64) {
                findings.push(DubbingAlignmentFinding {
                    code: DubbingAlignmentDiagnosticCode::SpeechLongerThanCue,
                    severity: AlignmentSeverity::Warning,
                    segment_id: segment.id.clone(),
                    message: format!(
                        "Speech duration ({:.2}s) exceeds subtitle cue window ({:.2}s) by {:.2}s",
                        trimmed_us as f64 / 1_000_000.0,
                        target_duration_us as f64 / 1_000_000.0,
                        diff_us as f64 / 1_000_000.0
                    ),
                    duration_diff_us: Some(diff_us),
                });
            } else if diff_us < -(config.tolerance_us as i64) {
                findings.push(DubbingAlignmentFinding {
                    code: DubbingAlignmentDiagnosticCode::SpeechShorterThanCue,
                    severity: AlignmentSeverity::Info,
                    segment_id: segment.id.clone(),
                    message: format!(
                        "Speech duration ({:.2}s) is shorter than subtitle cue window ({:.2}s) by {:.2}s",
                        trimmed_us as f64 / 1_000_000.0,
                        target_duration_us as f64 / 1_000_000.0,
                        (-diff_us) as f64 / 1_000_000.0
                    ),
                    duration_diff_us: Some(diff_us),
                });
            }

            // 2. Alignment plan diagnostics
            if let Some(plan) = &segment.alignment_plan {
                if plan.requires_manual_review {
                    findings.push(DubbingAlignmentFinding {
                        code: DubbingAlignmentDiagnosticCode::AlignmentRequiresReview,
                        severity: AlignmentSeverity::Warning,
                        segment_id: segment.id.clone(),
                        message: plan.selected_strategy.clone(),
                        duration_diff_us: Some(diff_us),
                    });
                }

                if plan.proposed_tempo > config.max_auto_tempo {
                    findings.push(DubbingAlignmentFinding {
                        code: DubbingAlignmentDiagnosticCode::TempoExceedsAutoLimit,
                        severity: AlignmentSeverity::Warning,
                        segment_id: segment.id.clone(),
                        message: format!(
                            "Proposed tempo {:.2}x exceeds safe AutoFit limit {:.2}x",
                            plan.proposed_tempo, config.max_auto_tempo
                        ),
                        duration_diff_us: Some(diff_us),
                    });
                } else if plan.proposed_tempo < config.min_auto_tempo {
                    findings.push(DubbingAlignmentFinding {
                        code: DubbingAlignmentDiagnosticCode::TempoBelowAutoLimit,
                        severity: AlignmentSeverity::Warning,
                        segment_id: segment.id.clone(),
                        message: format!(
                            "Proposed tempo {:.2}x is below safe AutoFit limit {:.2}x",
                            plan.proposed_tempo, config.min_auto_tempo
                        ),
                        duration_diff_us: Some(diff_us),
                    });
                }

                // Check for stale plan: source text hash or trim range changed
                if plan.source_trimmed_duration_us != trimmed_us {
                    findings.push(DubbingAlignmentFinding {
                        code: DubbingAlignmentDiagnosticCode::SourceTrimChanged,
                        severity: AlignmentSeverity::Warning,
                        segment_id: segment.id.clone(),
                        message: "Source audio trim interval has changed since alignment plan was generated".to_string(),
                        duration_diff_us: None,
                    });
                }
            }

            // 3. Alignment result diagnostics
            if let Some(result) = &segment.alignment_result {
                if let Some(aligned_id) = &result.aligned_asset_id {
                    if let Some(aligned_asset) = project.assets.get(aligned_id) {
                        if !aligned_asset.file_path.exists() {
                            findings.push(DubbingAlignmentFinding {
                                code: DubbingAlignmentDiagnosticCode::MissingAlignedAudio,
                                severity: AlignmentSeverity::Error,
                                segment_id: segment.id.clone(),
                                message: format!(
                                    "Aligned audio file not found on disk: {}",
                                    aligned_asset.file_name
                                ),
                                duration_diff_us: None,
                            });
                        }
                    } else {
                        findings.push(DubbingAlignmentFinding {
                            code: DubbingAlignmentDiagnosticCode::MissingAlignedAudio,
                            severity: AlignmentSeverity::Error,
                            segment_id: segment.id.clone(),
                            message: format!(
                                "Aligned asset ID '{aligned_id}' missing from project manifest"
                            ),
                            duration_diff_us: None,
                        });
                    }
                }
            }

            // 4. Track timeline collision check
            if let Some(prev_end) = prev_segment_end {
                if segment.timeline_start_us < prev_end {
                    let overlap = prev_end - segment.timeline_start_us;
                    findings.push(DubbingAlignmentFinding {
                        code: DubbingAlignmentDiagnosticCode::DialogueOverlap,
                        severity: AlignmentSeverity::Warning,
                        segment_id: segment.id.clone(),
                        message: format!(
                            "Segment overlaps with previous segment by {:.2}s",
                            overlap as f64 / 1_000_000.0
                        ),
                        duration_diff_us: Some(overlap as i64),
                    });
                }
            }

            // 5. Video duration boundary check
            if project.timeline_duration_us > 0
                && segment.timeline_end_us() > project.timeline_duration_us
            {
                findings.push(DubbingAlignmentFinding {
                    code: DubbingAlignmentDiagnosticCode::OutsideVideoBounds,
                    severity: AlignmentSeverity::Warning,
                    segment_id: segment.id.clone(),
                    message: format!(
                        "Segment end ({:.2}s) extends beyond video duration ({:.2}s)",
                        segment.timeline_end_us() as f64 / 1_000_000.0,
                        project.timeline_duration_us as f64 / 1_000_000.0
                    ),
                    duration_diff_us: None,
                });
            }

            prev_segment_end = Some(segment.timeline_end_us());
        }
    }

    findings
}
