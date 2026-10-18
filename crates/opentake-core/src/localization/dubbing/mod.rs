//! Professional AI Dubbing Timeline module for Open Media Workbench.
//!
//! Provides multi-track audio segment timeline editing, non-destructive trimming,
//! real waveform visualization, cue synchronization, and offline TTS placement.

pub mod errors;
pub mod history;
pub mod models;
pub mod operations;
pub mod persistence;
pub mod segments;
pub mod timeline;
pub mod tracks;
pub mod validation;
pub mod waveforms;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

pub use errors::DubbingError;
pub use history::DubbingHistory;
pub use models::{
    DiagnosticSeverity, DubbingAudioAsset, DubbingDiagnosticCode, DubbingDiagnosticFinding,
    DubbingProject, DubbingSegment, DubbingSegmentStatus, DubbingTimelineWaveform, DubbingTrack,
    DubbingWaveformBin, DUBBING_SCHEMA_VERSION,
};
pub use operations::{apply_operation, DubbingOperation};
pub use persistence::{
    get_dubbing_project_dir, load_dubbing_project_from_document, load_dubbing_project_from_file,
    save_dubbing_project_to_document, save_dubbing_project_to_file, DUBBING_METADATA_KEY,
};
pub use segments::{
    change_segment_track, delete_segment, duplicate_segment, move_segment, set_segment_gain,
    split_segment, trim_segment_end, trim_segment_start,
};
pub use timeline::{
    create_project_from_subtitles, get_dubbing_assets_dir, regenerate_segment,
    synthesize_and_place_cues,
};
pub use tracks::{
    add_track, remove_track, reorder_tracks, set_track_enabled, set_track_mute, set_track_solo,
    update_track_voice,
};
pub use validation::validate_dubbing_project;
pub use waveforms::generate_waveform_from_wav;

use crate::ai::storage::AiStorageManager;
use crate::ai::tts::engine::TtsEngine;
use crate::ai::tts::models::TtsVoiceSettings;
use crate::localization::subtitles::model::SubtitleDocument;

/// Central manager orchestrating timeline editing, synthesis, waveforms, and history.
#[derive(Clone)]
pub struct DubbingManager {
    storage: Arc<AiStorageManager>,
    engine: Arc<dyn TtsEngine>,
    base_dir: PathBuf,
    history: Arc<Mutex<HashMap<String, DubbingHistory>>>,
}

impl DubbingManager {
    pub fn new(
        storage: Arc<AiStorageManager>,
        engine: Arc<dyn TtsEngine>,
        base_dir: PathBuf,
    ) -> Self {
        Self {
            storage,
            engine,
            base_dir,
            history: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Constructs default local manager with system TTS engine.
    pub fn default_local(storage: Arc<AiStorageManager>) -> Self {
        let base_dir = storage.base_dir().to_path_buf();
        let engine = Arc::new(crate::ai::tts::engine::SystemPiperCliEngine::new(None));
        Self::new(storage, engine, base_dir)
    }

    pub fn storage(&self) -> &Arc<AiStorageManager> {
        &self.storage
    }

    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Retrieves an audio asset by its UUID.
    pub fn get_asset_file_path(&self, project: &DubbingProject, asset_id: &str) -> Option<PathBuf> {
        project.assets.get(asset_id).map(|a| a.file_path.clone())
    }

    /// Creates a new DubbingProject from a subtitle document track.
    pub fn create_project(
        &self,
        document: &SubtitleDocument,
        source_subtitle_track_id: &str,
        target_language: Option<String>,
    ) -> Result<DubbingProject, DubbingError> {
        create_project_from_subtitles(document, source_subtitle_track_id, target_language)
    }

    /// Synthesizes and places speech segments for subtitle cues on a dubbing track.
    #[allow(clippy::too_many_arguments)]
    pub async fn synthesize_track_cues(
        &self,
        project: &mut DubbingProject,
        dubbing_track_id: &str,
        document: &SubtitleDocument,
        cue_ids: Option<Vec<String>>,
        voice_override: Option<String>,
        speaker_override: Option<u32>,
        settings: Option<&TtsVoiceSettings>,
    ) -> Result<Vec<String>, DubbingError> {
        // Record undo state before synthesis changes
        self.record_undo_snapshot(project).await;

        let track = project
            .find_track(dubbing_track_id)
            .ok_or_else(|| DubbingError::TrackNotFound(dubbing_track_id.to_string()))?;

        let source_sub_track_id = track
            .source_subtitle_track_id
            .as_ref()
            .ok_or_else(|| DubbingError::TrackNotFound("No source subtitle track linked".into()))?;

        let sub_track = document
            .tracks
            .iter()
            .find(|t| &t.id == source_sub_track_id)
            .ok_or_else(|| DubbingError::TrackNotFound(source_sub_track_id.clone()))?;

        let voice_id = voice_override
            .or_else(|| track.assigned_voice_id.clone())
            .ok_or_else(|| DubbingError::VoiceUnavailable("No voice assigned to track".into()))?;

        let speaker_id = speaker_override.or(track.assigned_speaker_id);

        let cues_to_process: Vec<crate::localization::subtitles::model::SubtitleCue> = match cue_ids
        {
            Some(ids) => sub_track
                .cues
                .iter()
                .filter(|c| ids.contains(&c.id))
                .cloned()
                .collect(),
            None => sub_track.cues.clone(),
        };

        synthesize_and_place_cues(
            project,
            dubbing_track_id,
            &cues_to_process,
            &voice_id,
            speaker_id,
            self.engine.as_ref(),
            self.storage.as_ref(),
            &self.base_dir,
            settings,
        )
    }

    /// Regenerates a single segment.
    pub async fn regenerate_segment(
        &self,
        project: &mut DubbingProject,
        segment_id: &str,
        new_text: Option<String>,
        voice_override: Option<String>,
        speaker_override: Option<u32>,
        settings: Option<&TtsVoiceSettings>,
    ) -> Result<(), DubbingError> {
        self.record_undo_snapshot(project).await;

        regenerate_segment(
            project,
            segment_id,
            new_text,
            voice_override,
            speaker_override,
            self.engine.as_ref(),
            self.storage.as_ref(),
            &self.base_dir,
            settings,
        )
    }

    /// Applies an editing operation with undo history tracking.
    pub async fn execute_operation(
        &self,
        project: &mut DubbingProject,
        operation: &DubbingOperation,
    ) -> Result<(), DubbingError> {
        self.record_undo_snapshot(project).await;
        apply_operation(project, operation)
    }

    /// Undoes the last operation for the project.
    pub async fn undo(&self, project: &DubbingProject) -> Option<DubbingProject> {
        let mut hist_map = self.history.lock().await;
        let hist = hist_map.entry(project.id.clone()).or_default();
        hist.undo(project.clone())
    }

    /// Redoes the next undone operation for the project.
    pub async fn redo(&self, project: &DubbingProject) -> Option<DubbingProject> {
        let mut hist_map = self.history.lock().await;
        let hist = hist_map.entry(project.id.clone()).or_default();
        hist.redo(project.clone())
    }

    /// Generates or loads the waveform peak summary for a segment's audio asset.
    pub fn get_segment_waveform(
        &self,
        project: &DubbingProject,
        segment_id: &str,
    ) -> Result<DubbingTimelineWaveform, DubbingError> {
        let (_, segment) = project
            .find_segment(segment_id)
            .ok_or_else(|| DubbingError::SegmentNotFound(segment_id.to_string()))?;

        let asset = project
            .assets
            .get(&segment.audio_asset_id)
            .ok_or_else(|| DubbingError::AssetNotFound(segment.audio_asset_id.clone()))?;

        generate_waveform_from_wav(&asset.file_path, segment_id, &asset.asset_id)
    }

    /// Evaluates diagnostic warnings and issues.
    pub fn get_diagnostics(
        &self,
        project: &DubbingProject,
        document: Option<&SubtitleDocument>,
    ) -> Vec<DubbingDiagnosticFinding> {
        validate_dubbing_project(project, document)
    }

    /// Helper to record an undo snapshot.
    async fn record_undo_snapshot(&self, project: &DubbingProject) {
        let mut hist_map = self.history.lock().await;
        let hist = hist_map.entry(project.id.clone()).or_default();
        hist.record_snapshot(project);
    }
}
