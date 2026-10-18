//! Command pattern and operations dispatcher for dubbing timeline edits.

use serde::{Deserialize, Serialize};

use super::errors::DubbingError;
use super::models::DubbingProject;
use super::segments::{
    change_segment_track, delete_segment, duplicate_segment, move_segment, set_segment_gain,
    split_segment, trim_segment_end, trim_segment_start,
};
use super::tracks::{set_track_enabled, set_track_mute, set_track_solo, update_track_voice};

/// Serializable timeline editing operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[allow(clippy::large_enum_variant)]
pub enum DubbingOperation {
    MoveSegment {
        segment_id: String,
        new_timeline_start_us: u64,
    },
    TrimSegmentStart {
        segment_id: String,
        new_source_in_us: u64,
    },
    TrimSegmentEnd {
        segment_id: String,
        new_source_out_us: u64,
    },
    SplitSegment {
        segment_id: String,
        split_timeline_us: u64,
    },
    DeleteSegment {
        segment_id: String,
    },
    DuplicateSegment {
        segment_id: String,
        offset_us: u64,
    },
    ChangeTrack {
        segment_id: String,
        target_track_id: String,
    },
    SetGain {
        segment_id: String,
        gain: f32,
    },
    SetTrackMute {
        track_id: String,
        is_muted: bool,
    },
    SetTrackSolo {
        track_id: String,
        is_solo: bool,
    },
    SetTrackEnabled {
        track_id: String,
        is_enabled: bool,
    },
    UpdateTrackVoice {
        track_id: String,
        voice_id: Option<String>,
        speaker_id: Option<u32>,
    },
    ApplyAlignment {
        segment_id: String,
        plan: crate::localization::dubbing::alignment::models::DubbingAlignmentPlan,
        result: crate::localization::dubbing::alignment::models::DubbingAlignmentResult,
        aligned_asset: super::models::DubbingAudioAsset,
    },
    RestoreOriginalAudio {
        segment_id: String,
    },
    SetAlignmentMode {
        segment_id: String,
        mode: crate::localization::dubbing::alignment::models::DubbingAlignmentMode,
    },
    SetStartOffset {
        segment_id: String,
        offset_us: i64,
    },
    UpdateMixConfiguration {
        configuration: Box<crate::localization::dubbing::mixing::models::DubbingMixConfiguration>,
    },
    ApplyMixResult {
        result: Box<crate::localization::dubbing::mixing::models::DubbingMixResult>,
    },
    InvalidateMixResult,
}

/// Applies a timeline editing operation to the DubbingProject.
pub fn apply_operation(
    project: &mut DubbingProject,
    operation: &DubbingOperation,
) -> Result<(), DubbingError> {
    match operation {
        DubbingOperation::MoveSegment {
            segment_id,
            new_timeline_start_us,
        } => move_segment(project, segment_id, *new_timeline_start_us),

        DubbingOperation::TrimSegmentStart {
            segment_id,
            new_source_in_us,
        } => trim_segment_start(project, segment_id, *new_source_in_us),

        DubbingOperation::TrimSegmentEnd {
            segment_id,
            new_source_out_us,
        } => trim_segment_end(project, segment_id, *new_source_out_us),

        DubbingOperation::SplitSegment {
            segment_id,
            split_timeline_us,
        } => {
            split_segment(project, segment_id, *split_timeline_us)?;
            Ok(())
        }

        DubbingOperation::DeleteSegment { segment_id } => {
            delete_segment(project, segment_id)?;
            Ok(())
        }

        DubbingOperation::DuplicateSegment {
            segment_id,
            offset_us,
        } => {
            duplicate_segment(project, segment_id, *offset_us)?;
            Ok(())
        }

        DubbingOperation::ChangeTrack {
            segment_id,
            target_track_id,
        } => change_segment_track(project, segment_id, target_track_id),

        DubbingOperation::SetGain { segment_id, gain } => {
            set_segment_gain(project, segment_id, *gain)
        }

        DubbingOperation::SetTrackMute { track_id, is_muted } => {
            set_track_mute(project, track_id, *is_muted)
        }

        DubbingOperation::SetTrackSolo { track_id, is_solo } => {
            set_track_solo(project, track_id, *is_solo)
        }

        DubbingOperation::SetTrackEnabled {
            track_id,
            is_enabled,
        } => set_track_enabled(project, track_id, *is_enabled),

        DubbingOperation::UpdateTrackVoice {
            track_id,
            voice_id,
            speaker_id,
        } => update_track_voice(project, track_id, voice_id.clone(), *speaker_id),

        DubbingOperation::ApplyAlignment {
            segment_id,
            plan,
            result,
            aligned_asset,
        } => {
            // Register asset in project manifest
            project
                .assets
                .insert(aligned_asset.asset_id.clone(), aligned_asset.clone());

            let segment = project
                .find_segment_mut(segment_id)
                .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.clone()))?;

            if segment.original_audio_asset_id.is_none() {
                segment.original_audio_asset_id = Some(segment.audio_asset_id.clone());
            }

            segment.aligned_audio_asset_id = result.aligned_asset_id.clone();
            if let Some(aligned_id) = &result.aligned_asset_id {
                segment.audio_asset_id = aligned_id.clone();
            }
            segment.alignment_plan = Some(plan.clone());
            segment.alignment_result = Some(result.clone());
            segment.alignment_mode = Some(
                crate::localization::dubbing::alignment::models::DubbingAlignmentMode::AutoFit,
            );
            segment.start_offset_us = Some(plan.requested_start_offset_us);

            segment.source_in_us = 0;
            segment.source_out_us = result.actual_output_duration_us;

            if plan.requested_start_offset_us != 0 {
                segment.timeline_start_us = plan
                    .target_start_us
                    .saturating_add(plan.requested_start_offset_us.max(0) as u64);
            }

            let track_id = segment.track_id.clone();
            if let Some(track) = project.find_track_mut(&track_id) {
                track.sort_segments();
            }
            project.touch();
            Ok(())
        }

        DubbingOperation::RestoreOriginalAudio { segment_id } => {
            let orig_id_opt = {
                let (_track, segment) = project
                    .find_segment(segment_id)
                    .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.clone()))?;
                segment.original_audio_asset_id.clone()
            };

            let orig_duration = orig_id_opt
                .as_ref()
                .and_then(|id| project.assets.get(id))
                .map(|a| a.duration_us);

            let segment = project
                .find_segment_mut(segment_id)
                .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.clone()))?;

            if let Some(orig_id) = orig_id_opt {
                segment.audio_asset_id = orig_id;
                segment.alignment_mode = Some(
                    crate::localization::dubbing::alignment::models::DubbingAlignmentMode::Original,
                );
                if let Some(dur) = orig_duration {
                    segment.source_in_us = 0;
                    segment.source_out_us = dur;
                }
                if let Some(cue_start) = segment.original_cue_start_us {
                    segment.timeline_start_us = cue_start;
                }
                segment.start_offset_us = Some(0);
            }

            let track_id = segment.track_id.clone();
            if let Some(track) = project.find_track_mut(&track_id) {
                track.sort_segments();
            }
            project.touch();
            Ok(())
        }

        DubbingOperation::SetAlignmentMode { segment_id, mode } => {
            let active_id = {
                let segment = project
                    .find_segment_mut(segment_id)
                    .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.clone()))?;
                segment.alignment_mode = Some(*mode);
                segment.active_audio_asset_id().to_string()
            };

            let active_duration = project.assets.get(&active_id).map(|a| a.duration_us);

            let segment = project
                .find_segment_mut(segment_id)
                .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.clone()))?;

            segment.audio_asset_id = active_id;
            if let Some(dur) = active_duration {
                segment.source_in_us = 0;
                segment.source_out_us = dur;
            }

            let track_id = segment.track_id.clone();
            if let Some(track) = project.find_track_mut(&track_id) {
                track.sort_segments();
            }
            project.touch();
            Ok(())
        }

        DubbingOperation::SetStartOffset {
            segment_id,
            offset_us,
        } => {
            let segment = project
                .find_segment_mut(segment_id)
                .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.clone()))?;

            segment.start_offset_us = Some(*offset_us);
            let base_start = segment
                .original_cue_start_us
                .unwrap_or(segment.timeline_start_us);
            segment.timeline_start_us = if *offset_us >= 0 {
                base_start.saturating_add(*offset_us as u64)
            } else {
                base_start.saturating_sub((-offset_us) as u64)
            };

            let track_id = segment.track_id.clone();
            if let Some(track) = project.find_track_mut(&track_id) {
                track.sort_segments();
            }
            project.touch();
            Ok(())
        }

        DubbingOperation::UpdateMixConfiguration { configuration } => {
            project.mix_configuration = Some((**configuration).clone());
            // Changing mix settings invalidates previous rendered mixdown
            project.mix_result = None;
            project.touch();
            Ok(())
        }

        DubbingOperation::ApplyMixResult { result } => {
            project.mix_result = Some((**result).clone());
            project.touch();
            Ok(())
        }

        DubbingOperation::InvalidateMixResult => {
            project.mix_result = None;
            project.touch();
            Ok(())
        }
    }
}
