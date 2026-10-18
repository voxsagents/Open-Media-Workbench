//! Tauri IPC commands for the Professional AI Dubbing Timeline.

use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

use opentake_core::ai::storage::AiStorageManager;
use opentake_core::ai::tts::models::TtsVoiceSettings;
use opentake_core::localization::dubbing::models::{
    DubbingDiagnosticFinding, DubbingProject, DubbingTimelineWaveform,
};
use opentake_core::localization::dubbing::operations::DubbingOperation;
use opentake_core::localization::dubbing::persistence::{
    load_dubbing_project_from_document, load_dubbing_project_from_file,
    save_dubbing_project_to_document, save_dubbing_project_to_file,
};
use opentake_core::localization::dubbing::DubbingManager;
use opentake_core::localization::subtitles::model::SubtitleDocument;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixReadiness {
    pub is_ready: bool,
    pub has_original_audio: bool,
    pub original_audio_path: Option<String>,
    pub total_speech_segments: usize,
    pub ready_speech_segments: usize,
    pub missing_speech_segments: usize,
    pub has_existing_mix: bool,
    pub is_mix_stale: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixdownStatus {
    pub is_rendering: bool,
    pub progress: f32,
    pub message: String,
    pub error: Option<String>,
}

impl Default for DubbingMixdownStatus {
    fn default() -> Self {
        Self {
            is_rendering: false,
            progress: 0.0,
            message: "Idle".to_string(),
            error: None,
        }
    }
}

/// Managed state holding DubbingManager and the currently active editing project.
pub struct DubbingState {
    pub manager: Arc<DubbingManager>,
    pub active_project: Arc<RwLock<Option<DubbingProject>>>,
    pub mix_cancel_token: Arc<RwLock<Option<tokio_util::sync::CancellationToken>>>,
    pub mixdown_status: Arc<RwLock<DubbingMixdownStatus>>,
}

impl DubbingState {
    pub fn new(manager: Arc<DubbingManager>) -> Self {
        Self {
            manager,
            active_project: Arc::new(RwLock::new(None)),
            mix_cancel_token: Arc::new(RwLock::new(None)),
            mixdown_status: Arc::new(RwLock::new(DubbingMixdownStatus::default())),
        }
    }

    pub fn default_local(storage: Arc<AiStorageManager>) -> Self {
        let manager = Arc::new(DubbingManager::default_local(storage));
        Self::new(manager)
    }
}

/// Creates a new DubbingProject from a subtitle document track and sets it active.
#[tauri::command]
pub async fn create_dubbing_project(
    state: State<'_, DubbingState>,
    document: SubtitleDocument,
    source_subtitle_track_id: String,
    target_language: Option<String>,
) -> Result<DubbingProject, String> {
    let project = state
        .manager
        .create_project(&document, &source_subtitle_track_id, target_language)
        .map_err(|e| e.to_string())?;

    let mut guard = state.active_project.write().await;
    *guard = Some(project.clone());
    Ok(project)
}

/// Retrieves the currently active in-memory dubbing project.
#[tauri::command]
pub async fn get_active_dubbing_project(
    state: State<'_, DubbingState>,
) -> Result<Option<DubbingProject>, String> {
    let guard = state.active_project.read().await;
    Ok(guard.clone())
}

/// Sets or updates the currently active dubbing project.
#[tauri::command]
pub async fn set_active_dubbing_project(
    state: State<'_, DubbingState>,
    project: DubbingProject,
) -> Result<(), String> {
    let mut guard = state.active_project.write().await;
    *guard = Some(project);
    Ok(())
}

/// Loads an embedded dubbing project from SubtitleDocument metadata.
#[tauri::command]
pub async fn load_dubbing_from_document(
    state: State<'_, DubbingState>,
    document: SubtitleDocument,
) -> Result<Option<DubbingProject>, String> {
    let project_opt = load_dubbing_project_from_document(&document).map_err(|e| e.to_string())?;
    if let Some(ref proj) = project_opt {
        let mut guard = state.active_project.write().await;
        *guard = Some(proj.clone());
    }
    Ok(project_opt)
}

/// Embeds the active dubbing project into SubtitleDocument metadata and returns the updated document.
#[tauri::command]
pub async fn save_dubbing_to_document(
    state: State<'_, DubbingState>,
    mut document: SubtitleDocument,
) -> Result<SubtitleDocument, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project to save".into());
    };

    save_dubbing_project_to_document(&mut document, project).map_err(|e| e.to_string())?;
    Ok(document)
}

/// Saves the active dubbing project to a standalone JSON file.
#[tauri::command]
pub async fn save_dubbing_to_file(
    state: State<'_, DubbingState>,
    file_path: String,
) -> Result<(), String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project to save".into());
    };

    let path = PathBuf::from(file_path);
    save_dubbing_project_to_file(project, &path).map_err(|e| e.to_string())
}

/// Loads a dubbing project from a standalone JSON file and sets it active.
#[tauri::command]
pub async fn load_dubbing_from_file(
    state: State<'_, DubbingState>,
    file_path: String,
) -> Result<DubbingProject, String> {
    let path = PathBuf::from(file_path);
    let project = load_dubbing_project_from_file(&path).map_err(|e| e.to_string())?;

    let mut guard = state.active_project.write().await;
    *guard = Some(project.clone());
    Ok(project)
}

/// Synthesizes speech for cues on a dubbing track and places segments on the timeline.
#[tauri::command]
pub async fn generate_dubbing_segments(
    state: State<'_, DubbingState>,
    dubbing_track_id: String,
    document: SubtitleDocument,
    cue_ids: Option<Vec<String>>,
    voice_override: Option<String>,
    speaker_override: Option<u32>,
    settings: Option<TtsVoiceSettings>,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    state
        .manager
        .synthesize_track_cues(
            project,
            &dubbing_track_id,
            &document,
            cue_ids,
            voice_override,
            speaker_override,
            settings.as_ref(),
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Regenerates a single dubbing segment safely with rollback on failure.
#[tauri::command]
pub async fn regenerate_dubbing_segment(
    state: State<'_, DubbingState>,
    segment_id: String,
    new_text: Option<String>,
    voice_override: Option<String>,
    speaker_override: Option<u32>,
    settings: Option<TtsVoiceSettings>,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    state
        .manager
        .regenerate_segment(
            project,
            &segment_id,
            new_text,
            voice_override,
            speaker_override,
            settings.as_ref(),
        )
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Applies a non-destructive editing operation (Move, Trim, Split, Delete, Duplicate, Gain, Track changes).
#[tauri::command]
pub async fn apply_dubbing_operation(
    state: State<'_, DubbingState>,
    operation: DubbingOperation,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    state
        .manager
        .execute_operation(project, &operation)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Undoes the last editing operation.
#[tauri::command]
pub async fn undo_dubbing_operation(
    state: State<'_, DubbingState>,
) -> Result<Option<DubbingProject>, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref project) = *guard else {
        return Ok(None);
    };

    if let Some(undone) = state.manager.undo(project).await {
        *guard = Some(undone.clone());
        Ok(Some(undone))
    } else {
        Ok(None)
    }
}

/// Redoes the next undone editing operation.
#[tauri::command]
pub async fn redo_dubbing_operation(
    state: State<'_, DubbingState>,
) -> Result<Option<DubbingProject>, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref project) = *guard else {
        return Ok(None);
    };

    if let Some(redone) = state.manager.redo(project).await {
        *guard = Some(redone.clone());
        Ok(Some(redone))
    } else {
        Ok(None)
    }
}

/// Generates or returns the waveform summary for an audio segment.
#[tauri::command]
pub async fn get_dubbing_waveform(
    state: State<'_, DubbingState>,
    segment_id: String,
) -> Result<DubbingTimelineWaveform, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    state
        .manager
        .get_segment_waveform(project, &segment_id)
        .map_err(|e| e.to_string())
}

/// Computes diagnostic issues (missing assets, unresolved voices, overlaps, overruns, gaps).
#[tauri::command]
pub async fn get_dubbing_diagnostics(
    state: State<'_, DubbingState>,
    document: Option<SubtitleDocument>,
) -> Result<Vec<DubbingDiagnosticFinding>, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Ok(Vec::new());
    };

    Ok(state.manager.get_diagnostics(project, document.as_ref()))
}

/// Resolves an asset's playback URL (`opentake-media://localhost/{asset_id}`).
#[tauri::command]
pub async fn get_dubbing_asset_url(
    state: State<'_, DubbingState>,
    asset_id: String,
) -> Result<String, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    if project.assets.contains_key(&asset_id) {
        Ok(format!("opentake-media://localhost/{asset_id}"))
    } else {
        Err(format!("Asset '{asset_id}' not found in active project"))
    }
}

/// Generates an explainable duration alignment plan for a dubbing segment.
#[tauri::command]
pub async fn plan_dubbing_alignment(
    state: State<'_, DubbingState>,
    segment_id: String,
    config: Option<opentake_core::localization::dubbing::alignment::AlignmentConfig>,
    manual_tempo: Option<f32>,
    manual_padding: Option<opentake_core::localization::dubbing::alignment::SilencePaddingMode>,
    manual_offset_us: Option<i64>,
) -> Result<opentake_core::localization::dubbing::alignment::DubbingAlignmentPlan, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    state
        .manager
        .plan_segment_alignment(
            project,
            &segment_id,
            config.as_ref(),
            manual_tempo,
            manual_padding,
            manual_offset_us,
        )
        .map_err(|e| e.to_string())
}

/// Processes audio alignment for a planned segment using FFmpeg and silence padding.
#[tauri::command]
pub async fn process_dubbing_alignment(
    state: State<'_, DubbingState>,
    plan: opentake_core::localization::dubbing::alignment::DubbingAlignmentPlan,
) -> Result<
    (
        opentake_core::localization::dubbing::alignment::DubbingAlignmentResult,
        opentake_core::localization::dubbing::DubbingAudioAsset,
    ),
    String,
> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let cancel = tokio_util::sync::CancellationToken::new();
    state
        .manager
        .process_segment_alignment(project, &plan, cancel)
        .await
        .map_err(|e| e.to_string())
}

/// Applies an executed alignment result to the dubbing timeline.
#[tauri::command]
pub async fn apply_dubbing_alignment(
    state: State<'_, DubbingState>,
    segment_id: String,
    plan: opentake_core::localization::dubbing::alignment::DubbingAlignmentPlan,
    result: opentake_core::localization::dubbing::alignment::DubbingAlignmentResult,
    aligned_asset: opentake_core::localization::dubbing::DubbingAudioAsset,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let op = DubbingOperation::ApplyAlignment {
        segment_id,
        plan,
        result,
        aligned_asset,
    };

    state
        .manager
        .execute_operation(project, &op)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Restores the original TTS speech audio for a segment.
#[tauri::command]
pub async fn restore_original_dubbing_audio(
    state: State<'_, DubbingState>,
    segment_id: String,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let op = DubbingOperation::RestoreOriginalAudio { segment_id };

    state
        .manager
        .execute_operation(project, &op)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Sets the active playback mode (Original vs Aligned) for a segment.
#[tauri::command]
pub async fn set_dubbing_alignment_mode(
    state: State<'_, DubbingState>,
    segment_id: String,
    mode: opentake_core::localization::dubbing::alignment::DubbingAlignmentMode,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let op = DubbingOperation::SetAlignmentMode { segment_id, mode };

    state
        .manager
        .execute_operation(project, &op)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Adjusts manual start offset for a segment.
#[tauri::command]
pub async fn set_dubbing_start_offset(
    state: State<'_, DubbingState>,
    segment_id: String,
    offset_us: i64,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let op = DubbingOperation::SetStartOffset {
        segment_id,
        offset_us,
    };

    state
        .manager
        .execute_operation(project, &op)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Evaluates duration alignment diagnostics for the active project.
#[tauri::command]
pub async fn get_dubbing_alignment_diagnostics(
    state: State<'_, DubbingState>,
    config: Option<opentake_core::localization::dubbing::alignment::AlignmentConfig>,
) -> Result<Vec<opentake_core::localization::dubbing::alignment::DubbingAlignmentFinding>, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Ok(Vec::new());
    };

    Ok(state
        .manager
        .get_alignment_diagnostics(project, config.as_ref()))
}

/// Batch aligns selected or all eligible segments on dubbing tracks.
#[tauri::command]
pub async fn batch_align_dubbing_segments(
    state: State<'_, DubbingState>,
    segment_ids: Option<Vec<String>>,
    config: Option<opentake_core::localization::dubbing::alignment::AlignmentConfig>,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let eff_config = config.unwrap_or_default();

    // Identify candidate segments
    let candidate_ids: Vec<String> = match segment_ids {
        Some(ids) => ids,
        None => project
            .tracks
            .iter()
            .flat_map(|t| t.segments.iter().map(|s| s.id.clone()))
            .collect(),
    };

    let cancel = tokio_util::sync::CancellationToken::new();

    for seg_id in candidate_ids {
        let plan_res = state.manager.plan_segment_alignment(
            project,
            &seg_id,
            Some(&eff_config),
            None,
            None,
            None,
        );

        let Ok(plan) = plan_res else {
            continue;
        };

        // Skip items that require manual review in batch mode to prevent forced extreme distortion
        if plan.requires_manual_review {
            continue;
        }

        if let Ok((result, aligned_asset)) = state
            .manager
            .process_segment_alignment(project, &plan, cancel.clone())
            .await
        {
            let op = DubbingOperation::ApplyAlignment {
                segment_id: seg_id,
                plan,
                result,
                aligned_asset,
            };
            let _ = state.manager.execute_operation(project, &op).await;
        }
    }

    Ok(project.clone())
}

/// Retrieves the current mix configuration for the active dubbing project.
#[tauri::command]
pub async fn get_dubbing_mix_configuration(
    state: State<'_, DubbingState>,
) -> Result<opentake_core::localization::dubbing::mixing::DubbingMixConfiguration, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    Ok(state.manager.get_mix_configuration(project))
}

/// Updates the mix configuration and invalidates previous stale mixdown results.
#[tauri::command]
pub async fn update_dubbing_mix_configuration(
    state: State<'_, DubbingState>,
    configuration: opentake_core::localization::dubbing::mixing::DubbingMixConfiguration,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let op_update = DubbingOperation::UpdateMixConfiguration {
        configuration: Box::new(configuration),
    };
    state
        .manager
        .execute_operation(project, &op_update)
        .await
        .map_err(|e| e.to_string())?;

    let op_invalidate = DubbingOperation::InvalidateMixResult;
    state
        .manager
        .execute_operation(project, &op_invalidate)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}

/// Checks readiness for rendering an audio mixdown.
#[tauri::command]
pub async fn get_dubbing_mix_readiness(
    state: State<'_, DubbingState>,
    media_path_override: Option<String>,
) -> Result<DubbingMixReadiness, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let config = state.manager.get_mix_configuration(project);
    let resolved_media_path = media_path_override
        .map(PathBuf::from)
        .or_else(|| project.media_reference.as_ref().map(PathBuf::from));

    let mut warnings = Vec::new();
    let has_orig_audio = if let Some(ref mpath) = resolved_media_path {
        if mpath.exists() {
            let (has_audio, _, _) =
                opentake_core::localization::dubbing::mixing::probe_media_has_audio(mpath).await;
            has_audio
        } else {
            warnings.push(format!(
                "Source media '{}' not found on disk.",
                mpath.display()
            ));
            false
        }
    } else {
        false
    };

    let mut total_segments = 0;
    let mut ready_segments = 0;
    let mut missing_segments = 0;

    for track in &project.tracks {
        for seg in &track.segments {
            total_segments += 1;
            let active_asset_id = seg.active_audio_asset_id();
            if let Some(asset) = project.assets.get(active_asset_id) {
                if asset.file_path.exists() {
                    ready_segments += 1;
                } else {
                    missing_segments += 1;
                    warnings.push(format!(
                        "Asset '{}' file is missing on disk.",
                        asset.file_name
                    ));
                }
            } else {
                missing_segments += 1;
                warnings.push(format!(
                    "Segment '{}' has unresolved asset id '{}'.",
                    seg.id, active_asset_id
                ));
            }
        }
    }

    let has_existing_mix = project.mix_result.is_some();
    let is_mix_stale = project.mix_result.is_none() && has_existing_mix;

    let is_ready = (has_orig_audio || !config.original_audio_enabled)
        && missing_segments == 0
        && total_segments > 0;

    Ok(DubbingMixReadiness {
        is_ready,
        has_original_audio: has_orig_audio,
        original_audio_path: resolved_media_path.map(|p| p.to_string_lossy().to_string()),
        total_speech_segments: total_segments,
        ready_speech_segments: ready_segments,
        missing_speech_segments: missing_segments,
        has_existing_mix,
        is_mix_stale,
        warnings,
    })
}

/// Generates the deterministic mixdown plan.
#[tauri::command]
pub async fn plan_dubbing_mix(
    state: State<'_, DubbingState>,
    media_path_override: Option<String>,
) -> Result<opentake_core::localization::dubbing::mixing::DubbingMixPlan, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let override_path = media_path_override.as_ref().map(std::path::Path::new);
    state
        .manager
        .plan_mix(project, None, override_path)
        .await
        .map_err(|e| e.to_string())
}

/// Renders the complete multitrack audio mixdown to a durable PCM WAV file.
#[tauri::command]
pub async fn start_dubbing_mixdown(
    state: State<'_, DubbingState>,
    media_path_override: Option<String>,
) -> Result<opentake_core::localization::dubbing::mixing::DubbingMixResult, String> {
    let cancel = tokio_util::sync::CancellationToken::new();
    {
        let mut token_guard = state.mix_cancel_token.write().await;
        *token_guard = Some(cancel.clone());
        let mut status_guard = state.mixdown_status.write().await;
        *status_guard = DubbingMixdownStatus {
            is_rendering: true,
            progress: 0.1,
            message: "Constructing deterministic audio mix plan...".to_string(),
            error: None,
        };
    }

    let override_path = media_path_override.as_ref().map(PathBuf::from);

    let plan_res = {
        let guard = state.active_project.read().await;
        let Some(ref project) = *guard else {
            let mut status_guard = state.mixdown_status.write().await;
            status_guard.is_rendering = false;
            status_guard.error = Some("No active dubbing project".into());
            return Err("No active dubbing project".into());
        };
        state
            .manager
            .plan_mix(project, None, override_path.as_deref())
            .await
    };

    let plan = match plan_res {
        Ok(p) => p,
        Err(e) => {
            let mut status_guard = state.mixdown_status.write().await;
            status_guard.is_rendering = false;
            status_guard.error = Some(e.to_string());
            return Err(e.to_string());
        }
    };

    {
        let mut status_guard = state.mixdown_status.write().await;
        status_guard.progress = 0.4;
        status_guard.message = "Executing system FFmpeg multitrack audio mixdown...".to_string();
    }

    let render_res = state.manager.render_mixdown(&plan, cancel).await;

    let result = match render_res {
        Ok(r) => r,
        Err(e) => {
            let mut status_guard = state.mixdown_status.write().await;
            status_guard.is_rendering = false;
            status_guard.error = Some(e.to_string());
            return Err(e.to_string());
        }
    };

    {
        let mut guard = state.active_project.write().await;
        if let Some(ref mut project) = *guard {
            let op = DubbingOperation::ApplyMixResult {
                result: Box::new(result.clone()),
            };
            let _ = state.manager.execute_operation(project, &op).await;
        }

        let mut status_guard = state.mixdown_status.write().await;
        *status_guard = DubbingMixdownStatus {
            is_rendering: false,
            progress: 1.0,
            message: "Mixdown complete".to_string(),
            error: None,
        };
    }

    Ok(result)
}

/// Retrieves the status of an ongoing or completed mixdown.
#[tauri::command]
pub async fn get_dubbing_mixdown_status(
    state: State<'_, DubbingState>,
) -> Result<DubbingMixdownStatus, String> {
    let guard = state.mixdown_status.read().await;
    Ok(guard.clone())
}

/// Cancels an active mixdown rendering process.
#[tauri::command]
pub async fn cancel_dubbing_mixdown(state: State<'_, DubbingState>) -> Result<(), String> {
    let guard = state.mix_cancel_token.read().await;
    if let Some(ref token) = *guard {
        token.cancel();
    }
    let mut status_guard = state.mixdown_status.write().await;
    status_guard.is_rendering = false;
    status_guard.message = "Cancelled by user".to_string();
    Ok(())
}

/// Retrieves the currently rendered mixdown result, if present.
#[tauri::command]
pub async fn get_dubbing_mixdown_result(
    state: State<'_, DubbingState>,
) -> Result<Option<opentake_core::localization::dubbing::mixing::DubbingMixResult>, String> {
    let guard = state.active_project.read().await;
    let Some(ref project) = *guard else {
        return Err("No active dubbing project".into());
    };
    Ok(project.mix_result.clone())
}

/// Analyzes acoustic loudness, peaks, and clipping of a mixdown or audio asset.
#[tauri::command]
pub async fn analyze_dubbing_mix_audio(
    state: State<'_, DubbingState>,
    file_path: String,
) -> Result<opentake_core::localization::dubbing::mixing::AudioAnalysisReport, String> {
    let path = PathBuf::from(file_path);
    state
        .manager
        .analyze_mix_audio(&path)
        .map_err(|e| e.to_string())
}

/// Restores mixing configuration to a standard preset.
#[tauri::command]
pub async fn restore_dubbing_mix_configuration(
    state: State<'_, DubbingState>,
    preset: opentake_core::localization::dubbing::mixing::MixPreset,
) -> Result<DubbingProject, String> {
    let mut guard = state.active_project.write().await;
    let Some(ref mut project) = *guard else {
        return Err("No active dubbing project".into());
    };

    let mut config =
        opentake_core::localization::dubbing::mixing::DubbingMixConfiguration::new(&project.id);
    opentake_core::localization::dubbing::mixing::apply_preset_to_configuration(
        &mut config,
        preset,
    );

    let op_update = DubbingOperation::UpdateMixConfiguration {
        configuration: Box::new(config),
    };
    state
        .manager
        .execute_operation(project, &op_update)
        .await
        .map_err(|e| e.to_string())?;

    let op_invalidate = DubbingOperation::InvalidateMixResult;
    state
        .manager
        .execute_operation(project, &op_invalidate)
        .await
        .map_err(|e| e.to_string())?;

    Ok(project.clone())
}
