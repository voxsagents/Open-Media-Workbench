//! Professional Audio Mixing & Multitrack Mixdown Subsystem for AI Dubbing.
//!
//! Provides timeline-synchronized dialogue mixing, automatic original soundtrack ducking,
//! peak limiting, loudness analysis, non-destructive A/B preview, and durable WAV rendering.

pub mod analysis;
pub mod assets;
pub mod ducking;
pub mod errors;
pub mod filters;
pub mod models;
pub mod persistence;
pub mod planner;
pub mod presets;
pub mod renderer;

pub use analysis::{analyze_pcm_wav_file, AudioAnalysisReport};
pub use assets::{compute_mix_plan_cache_key, get_durable_mix_asset_path, promote_staged_mix_file};
pub use ducking::compute_ducking_intervals;
pub use errors::DubbingMixError;
pub use filters::{build_complex_filter_graph, build_ducking_volume_expression};
pub use models::{
    db_to_linear, linear_to_db, DubbingMixConfiguration, DubbingMixPlan, DubbingMixResult,
    DubbingMixSegment, DubbingMixTrackSettings, DuckingInterval, MixPreset,
    DUBBING_MIX_SCHEMA_VERSION,
};
pub use persistence::{
    invalidate_project_mix_result, set_project_mix_configuration, set_project_mix_result,
};
pub use planner::{plan_dubbing_mix, probe_media_has_audio};
pub use presets::apply_preset_to_configuration;
pub use renderer::render_dubbing_mixdown;
