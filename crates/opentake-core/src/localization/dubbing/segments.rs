//! Non-destructive segment editing operations and algorithms.

use uuid::Uuid;

use super::errors::DubbingError;
use super::models::{DubbingProject, DubbingSegment};

/// Moves a segment to a new timeline placement start position.
pub fn move_segment(
    project: &mut DubbingProject,
    segment_id: &str,
    new_timeline_start_us: u64,
) -> Result<(), DubbingError> {
    let segment = project
        .find_segment_mut(segment_id)
        .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

    segment.timeline_start_us = new_timeline_start_us;

    // Re-sort segments in the containing track
    let track_id = segment.track_id.clone();
    if let Some(track) = project.find_track_mut(&track_id) {
        track.sort_segments();
    }
    project.touch();
    Ok(())
}

/// Trims the start in-point of a segment non-destructively.
/// Advancing `source_in_us` shifts `timeline_start_us` forward by the same delta.
pub fn trim_segment_start(
    project: &mut DubbingProject,
    segment_id: &str,
    new_source_in_us: u64,
) -> Result<(), DubbingError> {
    let segment = project
        .find_segment_mut(segment_id)
        .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

    // Validate boundaries
    if new_source_in_us >= segment.source_out_us {
        return Err(DubbingError::InvalidTrim {
            source_in_us: new_source_in_us,
            source_out_us: segment.source_out_us,
            duration_us: segment.source_out_us,
        });
    }

    let delta_us = new_source_in_us.saturating_sub(segment.source_in_us);
    segment.source_in_us = new_source_in_us;
    segment.timeline_start_us = segment.timeline_start_us.saturating_add(delta_us);

    let track_id = segment.track_id.clone();
    if let Some(track) = project.find_track_mut(&track_id) {
        track.sort_segments();
    }
    project.touch();
    Ok(())
}

/// Trims the end out-point of a segment non-destructively.
pub fn trim_segment_end(
    project: &mut DubbingProject,
    segment_id: &str,
    new_source_out_us: u64,
) -> Result<(), DubbingError> {
    let asset_duration = {
        let segment = project
            .find_segment(segment_id)
            .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?
            .1;
        let asset = project
            .assets
            .get(&segment.audio_asset_id)
            .ok_or_else(|| DubbingError::AssetNotFound(segment.audio_asset_id.clone()))?;
        asset.duration_us
    };

    let segment = project
        .find_segment_mut(segment_id)
        .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

    if new_source_out_us <= segment.source_in_us || new_source_out_us > asset_duration {
        return Err(DubbingError::InvalidTrim {
            source_in_us: segment.source_in_us,
            source_out_us: new_source_out_us,
            duration_us: asset_duration,
        });
    }

    segment.source_out_us = new_source_out_us;

    let track_id = segment.track_id.clone();
    if let Some(track) = project.find_track_mut(&track_id) {
        track.sort_segments();
    }
    project.touch();
    Ok(())
}

/// Splits a segment at a given timeline position non-destructively.
/// Both segments share the underlying audio asset without modifying or copying the WAV file.
pub fn split_segment(
    project: &mut DubbingProject,
    segment_id: &str,
    split_timeline_us: u64,
) -> Result<(String, String), DubbingError> {
    let (track_id, original_segment) = {
        let (track, segment) = project
            .find_segment(segment_id)
            .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

        let start = segment.timeline_start_us;
        let end = segment.timeline_end_us();

        // Enforce split within interval with at least 50 ms margin
        const MIN_SEGMENT_US: u64 = 50_000;
        if split_timeline_us <= start + MIN_SEGMENT_US || split_timeline_us >= end - MIN_SEGMENT_US
        {
            return Err(DubbingError::InvalidSplit {
                split_timeline_us,
                start_us: start,
                end_us: end,
            });
        }

        (track.id.clone(), segment.clone())
    };

    let offset_from_start_us = split_timeline_us.saturating_sub(original_segment.timeline_start_us);
    let split_source_us = original_segment
        .source_in_us
        .saturating_add(offset_from_start_us);

    // Segment A (Left) keeps original ID
    let mut segment_a = original_segment.clone();
    segment_a.source_out_us = split_source_us;

    // Segment B (Right) gets a new UUID
    let id_b = Uuid::new_v4().to_string();
    let segment_b = DubbingSegment {
        id: id_b.clone(),
        track_id: track_id.clone(),
        source_subtitle_cue_id: original_segment.source_subtitle_cue_id.clone(),
        source_subtitle_track_id: original_segment.source_subtitle_track_id.clone(),
        source_text: original_segment.source_text.clone(),
        source_text_hash: original_segment.source_text_hash.clone(),
        voice_id: original_segment.voice_id.clone(),
        speaker_id: original_segment.speaker_id,
        audio_asset_id: original_segment.audio_asset_id.clone(),
        source_in_us: split_source_us,
        source_out_us: original_segment.source_out_us,
        timeline_start_us: split_timeline_us,
        original_cue_start_us: original_segment.original_cue_start_us,
        original_cue_end_us: original_segment.original_cue_end_us,
        gain: original_segment.gain,
        status: original_segment.status,
        warnings: original_segment.warnings.clone(),
    };

    let track = project
        .find_track_mut(&track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(track_id.clone()))?;

    // Replace original with Segment A, append Segment B
    if let Some(pos) = track
        .segments
        .iter()
        .position(|s| s.id == original_segment.id)
    {
        track.segments[pos] = segment_a;
    }
    track.segments.push(segment_b);
    track.sort_segments();

    project.touch();
    Ok((original_segment.id, id_b))
}

/// Deletes a segment from its containing track.
pub fn delete_segment(
    project: &mut DubbingProject,
    segment_id: &str,
) -> Result<DubbingSegment, DubbingError> {
    for track in &mut project.tracks {
        if let Some(pos) = track.segments.iter().position(|s| s.id == segment_id) {
            let removed = track.segments.remove(pos);
            project.touch();
            return Ok(removed);
        }
    }
    Err(DubbingError::SegmentNotFound(segment_id.to_string()))
}

/// Duplicates a segment on the same track with an offset.
pub fn duplicate_segment(
    project: &mut DubbingProject,
    segment_id: &str,
    offset_us: u64,
) -> Result<String, DubbingError> {
    let (track_id, original) = {
        let (track, segment) = project
            .find_segment(segment_id)
            .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;
        (track.id.clone(), segment.clone())
    };

    let new_id = Uuid::new_v4().to_string();
    let mut cloned = original;
    cloned.id = new_id.clone();
    cloned.timeline_start_us = cloned.timeline_start_us.saturating_add(offset_us);

    let track = project
        .find_track_mut(&track_id)
        .ok_or(DubbingError::TrackNotFound(track_id))?;

    track.segments.push(cloned);
    track.sort_segments();
    project.touch();

    Ok(new_id)
}

/// Moves a segment from one track to another compatible track.
pub fn change_segment_track(
    project: &mut DubbingProject,
    segment_id: &str,
    target_track_id: &str,
) -> Result<(), DubbingError> {
    // 1. Find segment and verify target track exists
    let (source_track_id, mut segment) = {
        let (track, seg) = project
            .find_segment(segment_id)
            .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;
        (track.id.clone(), seg.clone())
    };

    if source_track_id == target_track_id {
        return Ok(());
    }

    let target_track = project
        .find_track(target_track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(target_track_id.to_string()))?;

    // Check language compatibility if specified
    if !target_track.language.is_empty()
        && !segment.voice_id.is_empty()
        && !segment.voice_id.contains(&target_track.language)
        && !target_track
            .language
            .starts_with(&segment.voice_id[..2.min(segment.voice_id.len())])
    {
        // Non-blocking warning or validation if strict
    }

    // 2. Remove from source track
    if let Some(src_track) = project.find_track_mut(&source_track_id) {
        src_track.segments.retain(|s| s.id != segment_id);
    }

    // 3. Update track ID and insert into target track
    segment.track_id = target_track_id.to_string();
    let dst_track = project
        .find_track_mut(target_track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(target_track_id.to_string()))?;
    dst_track.segments.push(segment);
    dst_track.sort_segments();

    project.touch();
    Ok(())
}

/// Updates the gain setting on a segment.
pub fn set_segment_gain(
    project: &mut DubbingProject,
    segment_id: &str,
    gain: f32,
) -> Result<(), DubbingError> {
    let segment = project
        .find_segment_mut(segment_id)
        .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

    // Clamp gain between 0.0 and 2.0
    segment.gain = gain.clamp(0.0, 2.0);
    project.touch();
    Ok(())
}
