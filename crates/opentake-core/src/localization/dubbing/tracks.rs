//! Track management and track-level configuration.

use uuid::Uuid;

use super::errors::DubbingError;
use super::models::{DubbingProject, DubbingTrack};

/// Adds a new dubbing track to the project.
pub fn add_track(
    project: &mut DubbingProject,
    display_name: impl Into<String>,
    language: impl Into<String>,
    source_subtitle_track_id: Option<String>,
    assigned_voice_id: Option<String>,
    assigned_speaker_id: Option<u32>,
) -> String {
    let id = Uuid::new_v4().to_string();
    let mut track = DubbingTrack::new(id.clone(), display_name, language);
    track.source_subtitle_track_id = source_subtitle_track_id;
    track.assigned_voice_id = assigned_voice_id;
    track.assigned_speaker_id = assigned_speaker_id;

    project.tracks.push(track);
    project.touch();
    id
}

/// Removes a track and all its placed segments from the project.
pub fn remove_track(
    project: &mut DubbingProject,
    track_id: &str,
) -> Result<DubbingTrack, DubbingError> {
    if let Some(pos) = project.tracks.iter().position(|t| t.id == track_id) {
        let removed = project.tracks.remove(pos);
        project.touch();
        Ok(removed)
    } else {
        Err(DubbingError::TrackNotFound(track_id.to_string()))
    }
}

/// Reorders tracks within the project.
pub fn reorder_tracks(
    project: &mut DubbingProject,
    new_track_order_ids: &[String],
) -> Result<(), DubbingError> {
    if new_track_order_ids.len() != project.tracks.len() {
        return Err(DubbingError::TrackNotFound(
            "Track order length mismatch".into(),
        ));
    }

    let mut reordered = Vec::with_capacity(project.tracks.len());
    for id in new_track_order_ids {
        if let Some(pos) = project.tracks.iter().position(|t| &t.id == id) {
            reordered.push(project.tracks.remove(pos));
        } else {
            return Err(DubbingError::TrackNotFound(id.clone()));
        }
    }

    project.tracks = reordered;
    project.touch();
    Ok(())
}

/// Sets mute status for a track.
pub fn set_track_mute(
    project: &mut DubbingProject,
    track_id: &str,
    is_muted: bool,
) -> Result<(), DubbingError> {
    let track = project
        .find_track_mut(track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(track_id.to_string()))?;
    track.is_muted = is_muted;
    project.touch();
    Ok(())
}

/// Sets solo status for a track.
pub fn set_track_solo(
    project: &mut DubbingProject,
    track_id: &str,
    is_solo: bool,
) -> Result<(), DubbingError> {
    let track = project
        .find_track_mut(track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(track_id.to_string()))?;
    track.is_solo = is_solo;
    project.touch();
    Ok(())
}

/// Sets enabled status for a track.
pub fn set_track_enabled(
    project: &mut DubbingProject,
    track_id: &str,
    is_enabled: bool,
) -> Result<(), DubbingError> {
    let track = project
        .find_track_mut(track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(track_id.to_string()))?;
    track.is_enabled = is_enabled;
    project.touch();
    Ok(())
}

/// Updates the assigned voice and speaker variant for a track.
pub fn update_track_voice(
    project: &mut DubbingProject,
    track_id: &str,
    voice_id: Option<String>,
    speaker_id: Option<u32>,
) -> Result<(), DubbingError> {
    let track = project
        .find_track_mut(track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(track_id.to_string()))?;
    track.assigned_voice_id = voice_id;
    track.assigned_speaker_id = speaker_id;
    project.touch();
    Ok(())
}
