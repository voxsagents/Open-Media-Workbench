//! Core audio processing pipeline for Dubbing Duration Alignment.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::ai::tts::audio::validate_wav_file;
use crate::localization::dubbing::models::{DubbingAudioAsset, DubbingSegment};
use crate::localization::dubbing::timeline::get_dubbing_assets_dir;

use super::errors::DubbingAlignmentError;
use super::models::{
    DubbingAlignmentFinding, DubbingAlignmentPlan, DubbingAlignmentResult, DubbingAlignmentStatus,
};
use super::padding::pad_audio_with_silence;
use super::tempo::process_audio_tempo;

/// Computes a deterministic content-addressed cache key for an alignment operation.
pub fn compute_alignment_cache_key(
    original_asset_hash: &str,
    source_in_us: u64,
    source_out_us: u64,
    tempo: f32,
    pad_before_us: u64,
    pad_after_us: u64,
    algo_version: u32,
) -> String {
    let mut hasher = Sha256::new();
    let descriptor = format!(
        "{}:{}:{}:{:.4}:{}:{}:{}",
        original_asset_hash,
        source_in_us,
        source_out_us,
        tempo,
        pad_before_us,
        pad_after_us,
        algo_version
    );
    hasher.update(descriptor.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Executes alignment processing for a planned segment non-destructively.
pub async fn execute_alignment_plan(
    plan: &DubbingAlignmentPlan,
    segment: &DubbingSegment,
    asset: &DubbingAudioAsset,
    base_dir: &Path,
    cancel_token: CancellationToken,
) -> Result<(DubbingAlignmentResult, DubbingAudioAsset), DubbingAlignmentError> {
    let assets_dir = get_dubbing_assets_dir(base_dir);
    fs::create_dir_all(&assets_dir)?;

    let cache_key = compute_alignment_cache_key(
        &plan.original_asset_hash,
        segment.source_in_us,
        segment.source_out_us,
        plan.proposed_tempo,
        plan.padding_before_us,
        plan.padding_after_us,
        plan.algorithm_version,
    );

    let aligned_asset_id = format!("aligned_{}", &cache_key[..16]);
    let durable_file_name = format!("{aligned_asset_id}.wav");
    let durable_path = assets_dir.join(&durable_file_name);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // 1. Check if cached asset exists and is valid
    if durable_path.exists() {
        if let Ok(meta) = validate_wav_file(&durable_path) {
            let file_bytes = fs::read(&durable_path)?;
            let mut hasher = Sha256::new();
            hasher.update(&file_bytes);
            let sha256 = format!("{:x}", hasher.finalize());

            let aligned_asset = DubbingAudioAsset {
                asset_id: aligned_asset_id.clone(),
                file_path: durable_path,
                file_name: durable_file_name,
                sha256,
                sample_rate: meta.sample_rate,
                channel_count: meta.channel_count,
                duration_us: meta.duration_us,
                size_bytes: file_bytes.len() as u64,
                voice_id: asset.voice_id.clone(),
                speaker_id: asset.speaker_id,
                created_at_epoch_ms: now_ms,
            };

            let effective_start_us = plan
                .target_start_us
                .saturating_add(plan.requested_start_offset_us.max(0) as u64);
            let effective_end_us = effective_start_us.saturating_add(meta.duration_us);
            let measured_error_us =
                (meta.duration_us as i64) - (plan.effective_target_duration_us as i64);

            let result = DubbingAlignmentResult {
                alignment_id: Uuid::new_v4().to_string(),
                segment_id: plan.segment_id.clone(),
                original_asset_id: plan.original_asset_id.clone(),
                aligned_asset_id: Some(aligned_asset_id),
                actual_output_duration_us: meta.duration_us,
                effective_placement_start_us: effective_start_us,
                effective_placement_end_us: effective_end_us,
                processing_method: format!(
                    "cached (atempo={:.2}, pad_before={}ms, pad_after={}ms)",
                    plan.proposed_tempo,
                    plan.padding_before_us / 1000,
                    plan.padding_after_us / 1000
                ),
                measured_duration_error_us: measured_error_us,
                tempo_factor_applied: plan.proposed_tempo,
                padding_before_applied_us: plan.padding_before_us,
                padding_after_applied_us: plan.padding_after_us,
                diagnostics: Vec::new(),
                status: DubbingAlignmentStatus::Aligned,
                created_at_epoch_ms: now_ms,
            };

            return Ok((result, aligned_asset));
        }
    }

    // 2. Set up isolated staging directory
    let staging_dir = base_dir
        .join("staging")
        .join("alignment")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&staging_dir)?;

    let input_wav = &asset.file_path;
    if !input_wav.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(DubbingAlignmentError::AudioFileNotFound(
            input_wav.to_string_lossy().to_string(),
        ));
    }

    let needs_tempo = (plan.proposed_tempo - 1.0).abs() > 0.0001
        || segment.source_in_us > 0
        || segment.source_out_us < asset.duration_us;
    let needs_padding = plan.padding_before_us > 0 || plan.padding_after_us > 0;

    let staged_tempo_path = staging_dir.join("stage_tempo.wav");
    let staged_final_path = staging_dir.join("stage_final.wav");

    // 3. Step A: Tempo adjustment & source trimming via FFmpeg
    let intermediate_path = if needs_tempo {
        process_audio_tempo(
            input_wav,
            &staged_tempo_path,
            segment.source_in_us,
            segment.source_out_us,
            plan.proposed_tempo,
            asset.sample_rate,
            asset.channel_count,
            cancel_token.clone(),
        )
        .await?;

        staged_tempo_path
    } else {
        input_wav.clone()
    };

    if cancel_token.is_cancelled() {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(DubbingAlignmentError::Cancelled);
    }

    // 4. Step B: Silence padding
    let final_meta = if needs_padding {
        pad_audio_with_silence(
            &intermediate_path,
            &staged_final_path,
            plan.padding_before_us,
            plan.padding_after_us,
        )?
    } else if needs_tempo {
        validate_wav_file(&intermediate_path)
            .map_err(|e| DubbingAlignmentError::InvalidOutputWav(e.to_string()))?
    } else {
        // No tempo change and no padding: pure copy
        fs::copy(&intermediate_path, &staged_final_path)?;
        validate_wav_file(&staged_final_path)
            .map_err(|e| DubbingAlignmentError::InvalidOutputWav(e.to_string()))?
    };

    let source_for_promotion = if needs_padding || !needs_tempo {
        &staged_final_path
    } else {
        &intermediate_path
    };

    // 5. Promote into durable storage atomically
    fs::copy(source_for_promotion, &durable_path)?;
    let _ = fs::remove_dir_all(&staging_dir);

    let file_bytes = fs::read(&durable_path)?;
    let mut hasher = Sha256::new();
    hasher.update(&file_bytes);
    let sha256 = format!("{:x}", hasher.finalize());

    let aligned_asset = DubbingAudioAsset {
        asset_id: aligned_asset_id.clone(),
        file_path: durable_path,
        file_name: durable_file_name,
        sha256,
        sample_rate: final_meta.sample_rate,
        channel_count: final_meta.channel_count,
        duration_us: final_meta.duration_us,
        size_bytes: file_bytes.len() as u64,
        voice_id: asset.voice_id.clone(),
        speaker_id: asset.speaker_id,
        created_at_epoch_ms: now_ms,
    };

    let effective_start_us = plan
        .target_start_us
        .saturating_add(plan.requested_start_offset_us.max(0) as u64);
    let effective_end_us = effective_start_us.saturating_add(final_meta.duration_us);
    let measured_error_us =
        (final_meta.duration_us as i64) - (plan.effective_target_duration_us as i64);

    let mut diagnostics = Vec::new();
    if measured_error_us.abs() > 50_000 {
        diagnostics.push(DubbingAlignmentFinding {
            code: super::models::DubbingAlignmentDiagnosticCode::AlignedDurationMismatch,
            severity: super::models::AlignmentSeverity::Warning,
            segment_id: plan.segment_id.clone(),
            message: format!(
                "Aligned duration differs from target by {:.2}s",
                measured_error_us as f64 / 1_000_000.0
            ),
            duration_diff_us: Some(measured_error_us),
        });
    }

    let result = DubbingAlignmentResult {
        alignment_id: Uuid::new_v4().to_string(),
        segment_id: plan.segment_id.clone(),
        original_asset_id: plan.original_asset_id.clone(),
        aligned_asset_id: Some(aligned_asset_id),
        actual_output_duration_us: final_meta.duration_us,
        effective_placement_start_us: effective_start_us,
        effective_placement_end_us: effective_end_us,
        processing_method: format!(
            "atempo={:.4}, pad_before={}us, pad_after={}us",
            plan.proposed_tempo, plan.padding_before_us, plan.padding_after_us
        ),
        measured_duration_error_us: measured_error_us,
        tempo_factor_applied: plan.proposed_tempo,
        padding_before_applied_us: plan.padding_before_us,
        padding_after_applied_us: plan.padding_after_us,
        diagnostics,
        status: DubbingAlignmentStatus::Aligned,
        created_at_epoch_ms: now_ms,
    };

    Ok((result, aligned_asset))
}
