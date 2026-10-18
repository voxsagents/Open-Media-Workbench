//! Timeline construction, automatic cue placement, and speech synthesis workflows.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::errors::DubbingError;
use super::models::{
    DubbingAudioAsset, DubbingProject, DubbingSegment, DubbingSegmentStatus, DubbingTrack,
};
use crate::ai::storage::AiStorageManager;
use crate::ai::tts::audio::validate_wav_file;
use crate::ai::tts::engine::TtsEngine;
use crate::ai::tts::models::TtsVoiceSettings;
use crate::ai::voices::assignments::get_project_voice_assignments;
use crate::ai::voices::catalog::find_voice_by_id;
use crate::localization::subtitles::model::{SubtitleCue, SubtitleDocument};

/// Helper to compute SHA-256 hex digest of a byte slice.
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Helper to get the canonical audio asset directory for dubbing.
pub fn get_dubbing_assets_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("audio_assets").join("dubbing")
}

/// Initializes or creates a DubbingProject from a SubtitleDocument and a target SubtitleTrack.
pub fn create_project_from_subtitles(
    document: &SubtitleDocument,
    source_subtitle_track_id: &str,
    target_language: Option<String>,
) -> Result<DubbingProject, DubbingError> {
    let sub_track = document
        .tracks
        .iter()
        .find(|t| t.id == source_subtitle_track_id)
        .ok_or_else(|| DubbingError::TrackNotFound(source_subtitle_track_id.to_string()))?;

    let assignments = get_project_voice_assignments(document);
    let track_assignment = assignments
        .track_assignments
        .iter()
        .find(|a| a.track_id == source_subtitle_track_id);

    let assigned_voice_id = track_assignment
        .map(|a| a.voice_id.clone())
        .or_else(|| assignments.default_voice_id.clone());

    let assigned_speaker_id = track_assignment.and_then(|a| a.speaker_id);

    let target_lang = target_language.unwrap_or_else(|| sub_track.language.clone());
    let mut project = DubbingProject::new(
        document.id.clone(),
        sub_track.language.clone(),
        target_lang.clone(),
        document.associated_media_asset_id.clone(),
    );

    let mut dub_track = DubbingTrack::new(
        Uuid::new_v4().to_string(),
        format!("{} Dubbing", sub_track.display_name),
        target_lang,
    );
    dub_track.source_subtitle_track_id = Some(sub_track.id.clone());
    dub_track.assigned_voice_id = assigned_voice_id;
    dub_track.assigned_speaker_id = assigned_speaker_id;

    project.tracks.push(dub_track);
    project.timeline_duration_us = sub_track.duration_us().as_micros() as u64;

    Ok(project)
}

/// Generates durable speech audio and places timeline segments for specified cues.
#[allow(clippy::too_many_arguments)]
pub fn synthesize_and_place_cues(
    project: &mut DubbingProject,
    dubbing_track_id: &str,
    cues_to_synthesize: &[SubtitleCue],
    voice_id: &str,
    speaker_id: Option<u32>,
    engine: &dyn TtsEngine,
    storage: &AiStorageManager,
    base_dir: &Path,
    settings: Option<&TtsVoiceSettings>,
) -> Result<Vec<String>, DubbingError> {
    let assets_dir = get_dubbing_assets_dir(base_dir);
    fs::create_dir_all(&assets_dir)?;

    // 1. Resolve voice model and artifacts
    let voice_entry = find_voice_by_id(voice_id)
        .ok_or_else(|| DubbingError::VoiceUnavailable(voice_id.to_string()))?;

    if !storage.is_installed(&voice_entry.model_id) {
        return Err(DubbingError::VoiceUnavailable(format!(
            "Model '{}' for voice '{}' is not installed",
            voice_entry.model_id, voice_id
        )));
    }

    let manifest = crate::ai::catalog::find_catalog_model(&voice_entry.model_id)
        .ok_or_else(|| DubbingError::VoiceUnavailable(voice_entry.model_id.clone()))?;

    let model_dir = storage
        .resolve_model_dir(&voice_entry.model_id)
        .map_err(|e| DubbingError::VoiceUnavailable(e.to_string()))?;

    let model_path = model_dir.join(&manifest.artifact_filename);
    let config_artifact = manifest
        .additional_artifacts
        .iter()
        .find(|a| a.role == "config")
        .ok_or_else(|| {
            DubbingError::VoiceUnavailable(format!(
                "Voice configuration artifact missing for model '{}'",
                voice_entry.model_id
            ))
        })?;
    let config_path = model_dir.join(&config_artifact.filename);

    let default_settings = TtsVoiceSettings::default();
    let eff_settings = settings.unwrap_or(&default_settings);

    let _lock = storage
        .acquire_lock(&voice_entry.model_id)
        .map_err(|e| DubbingError::VoiceUnavailable(e.to_string()))?;

    let mut created_segment_ids = Vec::new();

    for cue in cues_to_synthesize {
        if cue.text.trim().is_empty() {
            continue;
        }

        let asset_id = Uuid::new_v4().to_string();
        let asset_file_name = format!("{asset_id}.wav");
        let asset_path = assets_dir.join(&asset_file_name);

        let synth_input = crate::ai::tts::models::TtsSynthesisInput {
            text: cue.text.clone(),
            voice_id: voice_id.to_string(),
            model_path: model_path.clone(),
            config_path: config_path.clone(),
            settings: TtsVoiceSettings {
                speaker_id,
                ..eff_settings.clone()
            },
        };

        // Synthesize via engine
        let metadata = engine
            .synthesize(&synth_input, &asset_path, None)
            .map_err(|e| DubbingError::TtsError(e.to_string()))?;

        // Strictly validate generated WAV
        let _ = validate_wav_file(&asset_path)
            .map_err(|e| DubbingError::AudioValidationFailed(e.to_string()))?;

        let file_bytes = fs::read(&asset_path)?;
        let sha256 = compute_sha256(&file_bytes);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Record durable asset in project manifest
        let audio_asset = DubbingAudioAsset {
            asset_id: asset_id.clone(),
            file_path: asset_path,
            file_name: asset_file_name,
            sha256,
            sample_rate: metadata.sample_rate,
            channel_count: metadata.channel_count,
            duration_us: metadata.duration_us,
            size_bytes: file_bytes.len() as u64,
            voice_id: voice_id.to_string(),
            speaker_id,
            created_at_epoch_ms: now_ms,
        };
        project.assets.insert(asset_id.clone(), audio_asset);

        // Create timeline segment placed at cue start time
        let seg_id = Uuid::new_v4().to_string();
        let cue_start_us = cue.start_us.as_micros() as u64;
        let cue_end_us = cue.end_us.as_micros() as u64;

        let segment = DubbingSegment {
            id: seg_id.clone(),
            track_id: dubbing_track_id.to_string(),
            source_subtitle_cue_id: Some(cue.id.clone()),
            source_subtitle_track_id: None,
            source_text: cue.text.clone(),
            source_text_hash: compute_sha256(cue.text.as_bytes()),
            voice_id: voice_id.to_string(),
            speaker_id,
            audio_asset_id: asset_id,
            source_in_us: 0,
            source_out_us: metadata.duration_us,
            timeline_start_us: cue_start_us,
            original_cue_start_us: Some(cue_start_us),
            original_cue_end_us: Some(cue_end_us),
            gain: 1.0,
            status: DubbingSegmentStatus::Ready,
            warnings: Vec::new(),
        };

        // If a segment already exists for this cue on the track, replace it; otherwise append
        let track = project
            .find_track_mut(dubbing_track_id)
            .ok_or_else(|| DubbingError::TrackNotFound(dubbing_track_id.to_string()))?;

        if let Some(pos) = track
            .segments
            .iter()
            .position(|s| s.source_subtitle_cue_id.as_deref() == Some(&cue.id))
        {
            track.segments[pos] = segment;
        } else {
            track.segments.push(segment);
        }

        created_segment_ids.push(seg_id);
    }

    // Sort segments on the track
    let track_dur = if let Some(track) = project.find_track_mut(dubbing_track_id) {
        track.sort_segments();
        track.duration_us()
    } else {
        0
    };
    project.timeline_duration_us = project.timeline_duration_us.max(track_dur);
    project.touch();

    Ok(created_segment_ids)
}

/// Regenerates a single segment with rollback preservation on failure.
#[allow(clippy::too_many_arguments)]
pub fn regenerate_segment(
    project: &mut DubbingProject,
    segment_id: &str,
    new_text: Option<String>,
    override_voice_id: Option<String>,
    override_speaker_id: Option<u32>,
    engine: &dyn TtsEngine,
    storage: &AiStorageManager,
    base_dir: &Path,
    settings: Option<&TtsVoiceSettings>,
) -> Result<(), DubbingError> {
    let (track_id, old_segment) = {
        let (track, seg) = project
            .find_segment(segment_id)
            .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;
        (track.id.clone(), seg.clone())
    };

    let text_to_synth = new_text.unwrap_or(old_segment.source_text);
    let voice_to_use = override_voice_id.unwrap_or(old_segment.voice_id);
    let speaker_to_use = override_speaker_id.or(old_segment.speaker_id);

    // Synthesize replacement into temporary asset
    let assets_dir = get_dubbing_assets_dir(base_dir);
    fs::create_dir_all(&assets_dir)?;

    let temp_asset_id = Uuid::new_v4().to_string();
    let temp_asset_file_name = format!("{temp_asset_id}.wav");
    let temp_asset_path = assets_dir.join(&temp_asset_file_name);

    let voice_entry = find_voice_by_id(&voice_to_use)
        .ok_or_else(|| DubbingError::VoiceUnavailable(voice_to_use.clone()))?;

    if !storage.is_installed(&voice_entry.model_id) {
        return Err(DubbingError::VoiceUnavailable(format!(
            "Model '{}' for voice '{}' is not installed",
            voice_entry.model_id, voice_to_use
        )));
    }

    let manifest = crate::ai::catalog::find_catalog_model(&voice_entry.model_id)
        .ok_or_else(|| DubbingError::VoiceUnavailable(voice_entry.model_id.clone()))?;

    let model_dir = storage
        .resolve_model_dir(&voice_entry.model_id)
        .map_err(|e| DubbingError::VoiceUnavailable(e.to_string()))?;

    let model_path = model_dir.join(&manifest.artifact_filename);
    let config_artifact = manifest
        .additional_artifacts
        .iter()
        .find(|a| a.role == "config")
        .ok_or_else(|| {
            DubbingError::VoiceUnavailable(format!(
                "Voice configuration artifact missing for model '{}'",
                voice_entry.model_id
            ))
        })?;
    let config_path = model_dir.join(&config_artifact.filename);

    let default_settings = TtsVoiceSettings::default();
    let eff_settings = settings.unwrap_or(&default_settings);

    let _lock = storage
        .acquire_lock(&voice_entry.model_id)
        .map_err(|e| DubbingError::VoiceUnavailable(e.to_string()))?;

    let synth_input = crate::ai::tts::models::TtsSynthesisInput {
        text: text_to_synth.clone(),
        voice_id: voice_to_use.clone(),
        model_path,
        config_path,
        settings: TtsVoiceSettings {
            speaker_id: speaker_to_use,
            ..eff_settings.clone()
        },
    };

    // Synthesis attempt: on failure, return without modifying project state
    let metadata = engine
        .synthesize(&synth_input, &temp_asset_path, None)
        .map_err(|e| DubbingError::TtsError(e.to_string()))?;

    // Validation attempt
    let _ = validate_wav_file(&temp_asset_path)
        .map_err(|e| DubbingError::AudioValidationFailed(e.to_string()))?;

    let file_bytes = fs::read(&temp_asset_path)?;
    let sha256 = compute_sha256(&file_bytes);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Register new asset
    let new_asset = DubbingAudioAsset {
        asset_id: temp_asset_id.clone(),
        file_path: temp_asset_path,
        file_name: temp_asset_file_name,
        sha256,
        sample_rate: metadata.sample_rate,
        channel_count: metadata.channel_count,
        duration_us: metadata.duration_us,
        size_bytes: file_bytes.len() as u64,
        voice_id: voice_to_use.clone(),
        speaker_id: speaker_to_use,
        created_at_epoch_ms: now_ms,
    };
    project.assets.insert(temp_asset_id.clone(), new_asset);

    // Update segment atomically
    let segment = project
        .find_segment_mut(segment_id)
        .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

    segment.source_text = text_to_synth.clone();
    segment.source_text_hash = compute_sha256(text_to_synth.as_bytes());
    segment.voice_id = voice_to_use;
    segment.speaker_id = speaker_to_use;
    segment.audio_asset_id = temp_asset_id;
    segment.source_in_us = 0;
    segment.source_out_us = metadata.duration_us;
    segment.status = DubbingSegmentStatus::Ready;
    segment.warnings.clear();

    if let Some(track) = project.find_track_mut(&track_id) {
        track.sort_segments();
    }
    project.touch();

    Ok(())
}
