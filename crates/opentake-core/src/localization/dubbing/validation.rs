//! Timeline diagnostics and validation rules.

use super::models::{
    DiagnosticSeverity, DubbingDiagnosticCode, DubbingDiagnosticFinding, DubbingProject,
};
use crate::localization::subtitles::model::SubtitleDocument;

/// Evaluates all diagnostic checks across a dubbing project.
pub fn validate_dubbing_project(
    project: &DubbingProject,
    document: Option<&SubtitleDocument>,
) -> Vec<DubbingDiagnosticFinding> {
    let mut findings = Vec::new();

    for track in &project.tracks {
        // 1. Check Voice Assignment
        if track.assigned_voice_id.is_none() {
            findings.push(DubbingDiagnosticFinding {
                code: DubbingDiagnosticCode::UnresolvedVoiceAssignment,
                severity: DiagnosticSeverity::Warning,
                track_id: Some(track.id.clone()),
                segment_id: None,
                cue_id: None,
                message: format!("Track '{}' has no assigned TTS voice.", track.display_name),
                time_range_us: None,
            });
        }

        // 2. Track-level Segments Loop
        for i in 0..track.segments.len() {
            let seg = &track.segments[i];

            // 2a. Check Missing Audio Asset
            if let Some(asset) = project.assets.get(&seg.audio_asset_id) {
                if !asset.file_path.exists() {
                    findings.push(DubbingDiagnosticFinding {
                        code: DubbingDiagnosticCode::MissingAudioAsset,
                        severity: DiagnosticSeverity::Error,
                        track_id: Some(track.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        cue_id: seg.source_subtitle_cue_id.clone(),
                        message: format!(
                            "Audio asset file is missing on disk: {}",
                            asset.file_path.display()
                        ),
                        time_range_us: Some((seg.timeline_start_us, seg.timeline_end_us())),
                    });
                }
            } else {
                findings.push(DubbingDiagnosticFinding {
                    code: DubbingDiagnosticCode::MissingAudioAsset,
                    severity: DiagnosticSeverity::Error,
                    track_id: Some(track.id.clone()),
                    segment_id: Some(seg.id.clone()),
                    cue_id: seg.source_subtitle_cue_id.clone(),
                    message: format!(
                        "Audio asset ID '{}' not found in project manifest.",
                        seg.audio_asset_id
                    ),
                    time_range_us: Some((seg.timeline_start_us, seg.timeline_end_us())),
                });
            }

            // 2b. Check Source Trim Validity
            if seg.source_in_us >= seg.source_out_us {
                findings.push(DubbingDiagnosticFinding {
                    code: DubbingDiagnosticCode::InvalidSourceTrim,
                    severity: DiagnosticSeverity::Error,
                    track_id: Some(track.id.clone()),
                    segment_id: Some(seg.id.clone()),
                    cue_id: seg.source_subtitle_cue_id.clone(),
                    message: format!(
                        "Invalid source trim: in ({}) >= out ({})",
                        seg.source_in_us, seg.source_out_us
                    ),
                    time_range_us: Some((seg.timeline_start_us, seg.timeline_end_us())),
                });
            }

            // 2c. Check Cue Overrun
            if let Some(overrun) = seg.cue_overrun_us() {
                // Warning threshold: > 500 ms (500,000 µs)
                if overrun > 500_000 {
                    findings.push(DubbingDiagnosticFinding {
                        code: DubbingDiagnosticCode::SegmentExceedsSubtitleInterval,
                        severity: DiagnosticSeverity::Warning,
                        track_id: Some(track.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        cue_id: seg.source_subtitle_cue_id.clone(),
                        message: format!(
                            "Speech segment overruns subtitle cue by {:.2}s.",
                            overrun as f64 / 1_000_000.0
                        ),
                        time_range_us: Some((seg.timeline_start_us, seg.timeline_end_us())),
                    });
                }
            }

            // 2d. Check Overlap with Next Segment
            if i + 1 < track.segments.len() {
                let next_seg = &track.segments[i + 1];
                let cur_end = seg.timeline_end_us();
                if cur_end > next_seg.timeline_start_us {
                    let overlap_us = cur_end - next_seg.timeline_start_us;
                    findings.push(DubbingDiagnosticFinding {
                        code: DubbingDiagnosticCode::SegmentOverlap,
                        severity: DiagnosticSeverity::Warning,
                        track_id: Some(track.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        cue_id: seg.source_subtitle_cue_id.clone(),
                        message: format!(
                            "Speech segment overlaps with subsequent segment by {:.2}s.",
                            overlap_us as f64 / 1_000_000.0
                        ),
                        time_range_us: Some((next_seg.timeline_start_us, cur_end)),
                    });
                } else if next_seg.timeline_start_us > cur_end + 15_000_000 {
                    // Gap > 15 seconds: Information level
                    findings.push(DubbingDiagnosticFinding {
                        code: DubbingDiagnosticCode::LargeDialogueGap,
                        severity: DiagnosticSeverity::Info,
                        track_id: Some(track.id.clone()),
                        segment_id: Some(seg.id.clone()),
                        cue_id: seg.source_subtitle_cue_id.clone(),
                        message: format!(
                            "Large silence gap ({:.1}s) between speech segments.",
                            (next_seg.timeline_start_us - cur_end) as f64 / 1_000_000.0
                        ),
                        time_range_us: Some((cur_end, next_seg.timeline_start_us)),
                    });
                }
            }
        }

        // 3. Subtitle Cue Synchronization Check (if SubtitleDocument provided)
        if let Some(doc) = document {
            if let Some(sub_track_id) = &track.source_subtitle_track_id {
                if let Some(sub_track) = doc.tracks.iter().find(|t| &t.id == sub_track_id) {
                    for cue in &sub_track.cues {
                        let has_segment = track
                            .segments
                            .iter()
                            .any(|s| s.source_subtitle_cue_id.as_deref() == Some(&cue.id));
                        if !has_segment {
                            findings.push(DubbingDiagnosticFinding {
                                code: DubbingDiagnosticCode::MissingAudioSegment,
                                severity: DiagnosticSeverity::Info,
                                track_id: Some(track.id.clone()),
                                segment_id: None,
                                cue_id: Some(cue.id.clone()),
                                message: format!("Cue '{}' has not been synthesized yet.", cue.id),
                                time_range_us: Some((
                                    cue.start_us.as_micros() as u64,
                                    cue.end_us.as_micros() as u64,
                                )),
                            });
                        }
                    }
                }
            }
        }
    }

    findings
}
