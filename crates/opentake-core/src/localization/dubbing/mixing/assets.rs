//! Durable asset storage and cache key management for mixed audio.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::models::DubbingMixPlan;

/// Generates a content-addressed SHA-256 fingerprint for a mixing plan.
pub fn compute_mix_plan_cache_key(plan: &DubbingMixPlan) -> String {
    let mut hasher = Sha256::new();

    hasher.update(plan.project_id.as_bytes());
    hasher.update(plan.timeline_duration_us.to_le_bytes());
    hasher.update(plan.has_original_audio.to_string().as_bytes());

    if let Some(ref orig_path) = plan.original_audio_path {
        hasher.update(orig_path.to_string_lossy().as_bytes());
    }

    hasher.update(plan.original_gain_db.to_le_bytes());
    hasher.update(plan.dubbing_master_gain_db.to_le_bytes());

    for seg in &plan.segments {
        hasher.update(seg.segment_id.as_bytes());
        hasher.update(seg.asset_id.as_bytes());
        hasher.update(seg.timeline_start_us.to_le_bytes());
        hasher.update(seg.duration_us.to_le_bytes());
        hasher.update(seg.source_in_us.to_le_bytes());
        hasher.update(seg.source_out_us.to_le_bytes());
        hasher.update(seg.gain_db.to_le_bytes());
        hasher.update(seg.is_aligned.to_string().as_bytes());
    }

    for interval in &plan.ducking_intervals {
        hasher.update(interval.start_us.to_le_bytes());
        hasher.update(interval.end_us.to_le_bytes());
        hasher.update(interval.attack_us.to_le_bytes());
        hasher.update(interval.release_us.to_le_bytes());
        hasher.update(interval.attenuation_db.to_le_bytes());
    }

    hasher.update(plan.output_sample_rate.to_le_bytes());
    hasher.update(plan.output_channels.to_le_bytes());

    // Algorithm version 1
    hasher.update(b"v1");

    format!("{:x}", hasher.finalize())
}

/// Returns the durable file path for a mix asset with the given cache key.
pub fn get_durable_mix_asset_path(base_dir: &Path, cache_key: &str) -> PathBuf {
    base_dir
        .join("dubbing")
        .join("assets")
        .join(format!("mix_{cache_key}.wav"))
}

/// Atomically promotes a staged mixdown WAV file into durable asset storage.
pub fn promote_staged_mix_file(
    staged_path: &Path,
    durable_path: &Path,
) -> Result<String, std::io::Error> {
    if let Some(parent) = durable_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if durable_path.exists() {
        let _ = std::fs::remove_file(durable_path);
    }

    std::fs::rename(staged_path, durable_path)?;

    // Compute final file SHA-256
    let bytes = std::fs::read(durable_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
