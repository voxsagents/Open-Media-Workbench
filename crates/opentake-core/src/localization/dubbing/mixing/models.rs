//! Canonical data contracts and structures for AI Dubbing Audio Mixing & Mixdown.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Current schema version for DubbingMixConfiguration.
pub const DUBBING_MIX_SCHEMA_VERSION: u32 = 1;

/// Curated professional audio mixing presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum MixPreset {
    /// Balanced mix: moderate ducking (-12dB), subtle soundtrack reduction (-6dB).
    #[default]
    Balanced,
    /// Dialogue prominence: aggressive ducking (-18dB), prominent voice (+1.5dB).
    VoiceFocus,
    /// Soundtrack preservation: light ducking (-6dB), original soundtrack at 0dB.
    PreserveOriginal,
    /// Custom user-defined configuration.
    Custom,
}

/// Track-specific mixing parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixTrackSettings {
    pub track_id: String,
    pub is_enabled: bool,
    pub is_muted: bool,
    pub is_solo: bool,
    /// Track gain adjustment in decibels (e.g. 0.0 dB).
    pub gain_db: f32,
    /// Stereo panning from -1.0 (full left) to +1.0 (full right). Default 0.0 (center).
    pub pan: f32,
}

impl DubbingMixTrackSettings {
    pub fn new(track_id: impl Into<String>) -> Self {
        Self {
            track_id: track_id.into(),
            is_enabled: true,
            is_muted: false,
            is_solo: false,
            gain_db: 0.0,
            pan: 0.0,
        }
    }
}

/// Comprehensive project audio mixing configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixConfiguration {
    pub schema_version: u32,
    pub project_id: String,
    /// Whether the original video soundtrack is included in the mixdown.
    pub original_audio_enabled: bool,
    /// Original soundtrack gain adjustment in decibels (e.g. -6.0 dB).
    pub original_audio_gain_db: f32,
    /// Master dubbing voice tracks gain adjustment in decibels (e.g. 0.0 dB).
    pub dubbing_master_gain_db: f32,
    /// Per-track settings override.
    pub track_settings: HashMap<String, DubbingMixTrackSettings>,
    /// Whether automatic dialogue ducking of original soundtrack is enabled.
    pub ducking_enabled: bool,
    /// Attenuation level during active speech in decibels (e.g. -12.0 dB).
    pub ducking_attenuation_db: f32,
    /// Attack duration before speech onset in milliseconds (e.g. 50 ms).
    pub ducking_attack_ms: u32,
    /// Release duration after speech offset in milliseconds (e.g. 250 ms).
    pub ducking_release_ms: u32,
    /// Hold duration before release ramps up in milliseconds (e.g. 100 ms).
    pub ducking_hold_ms: u32,
    /// Original soundtrack fade-in duration in milliseconds.
    pub fade_in_ms: u32,
    /// Original soundtrack fade-out duration in milliseconds.
    pub fade_out_ms: u32,
    /// Output sample rate in Hz (default 48000).
    pub output_sample_rate: u32,
    /// Output channel count (1 for mono, 2 for stereo; default 2).
    pub output_channels: u16,
    /// Currently selected preset.
    pub preset: MixPreset,
    /// Whether brickwall peak limiter is enabled on final mix summation.
    pub limiter_enabled: bool,
    /// Peak ceiling in dBFS (e.g. -1.0 dBFS).
    pub peak_ceiling_db: f32,
    /// Timestamp of last modification in epoch milliseconds.
    pub updated_at_epoch_ms: u64,
}

impl Default for DubbingMixConfiguration {
    fn default() -> Self {
        Self::new_with_preset("default", MixPreset::Balanced)
    }
}

impl DubbingMixConfiguration {
    pub fn new(project_id: impl Into<String>) -> Self {
        Self::new_with_preset(project_id, MixPreset::Balanced)
    }

    pub fn new_with_preset(project_id: impl Into<String>, preset: MixPreset) -> Self {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let (orig_gain, dub_gain, duck_att, att_ms, rel_ms, hold_ms) = match preset {
            MixPreset::Balanced => (-6.0, 0.0, -12.0, 50, 250, 100),
            MixPreset::VoiceFocus => (-12.0, 1.5, -18.0, 30, 300, 150),
            MixPreset::PreserveOriginal => (0.0, -1.0, -6.0, 80, 200, 80),
            MixPreset::Custom => (-6.0, 0.0, -12.0, 50, 250, 100),
        };

        Self {
            schema_version: DUBBING_MIX_SCHEMA_VERSION,
            project_id: project_id.into(),
            original_audio_enabled: true,
            original_audio_gain_db: orig_gain,
            dubbing_master_gain_db: dub_gain,
            track_settings: HashMap::new(),
            ducking_enabled: true,
            ducking_attenuation_db: duck_att,
            ducking_attack_ms: att_ms,
            ducking_release_ms: rel_ms,
            ducking_hold_ms: hold_ms,
            fade_in_ms: 0,
            fade_out_ms: 0,
            output_sample_rate: 48000,
            output_channels: 2,
            preset,
            limiter_enabled: true,
            peak_ceiling_db: -1.0,
            updated_at_epoch_ms: now_ms,
        }
    }

    pub fn touch(&mut self) {
        self.updated_at_epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
    }
}

/// Represents an active speech segment resolved for mixing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixSegment {
    pub segment_id: String,
    pub track_id: String,
    pub asset_id: String,
    pub resolved_file_path: PathBuf,
    pub timeline_start_us: u64,
    pub duration_us: u64,
    pub source_in_us: u64,
    pub source_out_us: u64,
    pub gain_db: f32,
    pub fade_in_ms: u32,
    pub fade_out_ms: u32,
    pub is_aligned: bool,
}

/// Computed interval during which original soundtrack is ducked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuckingInterval {
    pub start_us: u64,
    pub end_us: u64,
    pub attack_us: u64,
    pub release_us: u64,
    pub attenuation_db: f32,
}

/// Complete deterministic blueprint for executing an FFmpeg mixdown.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixPlan {
    pub plan_id: String,
    pub project_id: String,
    pub timeline_duration_us: u64,
    pub has_original_audio: bool,
    pub original_audio_path: Option<PathBuf>,
    pub original_audio_sample_rate: u32,
    pub original_audio_channels: u16,
    pub original_gain_db: f32,
    pub dubbing_master_gain_db: f32,
    pub segments: Vec<DubbingMixSegment>,
    pub ducking_intervals: Vec<DuckingInterval>,
    pub output_sample_rate: u32,
    pub output_channels: u16,
    pub filter_graph_desc: String,
    pub warnings: Vec<String>,
}

/// Verification and acoustic quality metrics of a rendered mixdown WAV asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DubbingMixResult {
    pub result_id: String,
    pub project_id: String,
    pub mix_asset_id: String,
    pub file_path: PathBuf,
    pub sha256: String,
    pub actual_duration_us: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
    pub file_size_bytes: u64,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub integrated_lufs: Option<f32>,
    pub clipping_samples: u64,
    pub warnings: Vec<String>,
    pub created_at_epoch_ms: u64,
}

/// Converts decibel value to linear amplitude multiplier (clamped to 0.0 for <= -99 dB).
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= -99.0 {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

/// Converts linear amplitude multiplier to decibels (clamped at -100 dB).
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.00001 {
        -100.0
    } else {
        20.0 * linear.log10()
    }
}
