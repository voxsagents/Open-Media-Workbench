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
    }
}
