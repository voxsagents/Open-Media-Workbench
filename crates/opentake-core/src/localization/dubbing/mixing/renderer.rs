//! FFmpeg mixdown rendering pipeline: staging, execution, validation, and durable promotion.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::media::discovery::{discover_tool, DiscoveryConfig, ToolType};
use crate::media::process::{run_managed_process, ProcessSpec};

use super::analysis::analyze_pcm_wav_file;
use super::assets::{
    compute_mix_plan_cache_key, get_durable_mix_asset_path, promote_staged_mix_file,
};
use super::errors::DubbingMixError;
use super::models::{DubbingMixPlan, DubbingMixResult};

const DEFAULT_RENDER_TIMEOUT: Duration = Duration::from_secs(300);

/// Executes the full offline mixdown pipeline.
pub async fn render_dubbing_mixdown(
    plan: &DubbingMixPlan,
    base_dir: &Path,
    cancel_token: CancellationToken,
) -> Result<DubbingMixResult, DubbingMixError> {
    let cache_key = compute_mix_plan_cache_key(plan);
    let durable_path = get_durable_mix_asset_path(base_dir, &cache_key);

    // 1. Check content-addressed cache
    if durable_path.exists() {
        if let Ok(report) = analyze_pcm_wav_file(&durable_path) {
            let metadata = std::fs::metadata(&durable_path)?;
            let bytes = std::fs::read(&durable_path)?;
            let mut hasher = sha2::Sha256::default();
            sha2::Digest::update(&mut hasher, &bytes);
            let sha256 = format!("{:x}", sha2::Digest::finalize(hasher));

            return Ok(DubbingMixResult {
                result_id: Uuid::new_v4().to_string(),
                project_id: plan.project_id.clone(),
                mix_asset_id: format!("mix_{cache_key}"),
                file_path: durable_path,
                sha256,
                actual_duration_us: report.duration_us,
                sample_rate: report.sample_rate,
                channels: report.channels,
                bit_depth: report.bit_depth,
                file_size_bytes: metadata.len(),
                peak_dbfs: report.peak_dbfs,
                rms_dbfs: report.rms_dbfs,
                integrated_lufs: report.integrated_lufs,
                clipping_samples: report.clipping_samples,
                warnings: plan.warnings.clone(),
                created_at_epoch_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
            });
        }
    }

    // 2. Setup staging path
    let staging_dir = base_dir
        .join("staging")
        .join("mixdown")
        .join(Uuid::new_v4().to_string());
    tokio::fs::create_dir_all(&staging_dir)
        .await
        .map_err(DubbingMixError::IoError)?;
    let staged_path = staging_dir.join("mixed_output.wav");

    // 3. Locate system FFmpeg binary
    let discovered = discover_tool(ToolType::Ffmpeg, &DiscoveryConfig::default()).map_err(|e| {
        DubbingMixError::FFmpegExecutionFailed(format!(
            "System FFmpeg binary was not found in standard system paths: {e}"
        ))
    })?;

    // 4. Build command arguments
    let mut args: Vec<String> = Vec::new();
    args.push("-y".to_string());
    args.push("-v".to_string());
    args.push("error".to_string());

    // Input 0: original audio (if present)
    if plan.has_original_audio {
        if let Some(ref orig_path) = plan.original_audio_path {
            args.push("-i".to_string());
            args.push(orig_path.to_string_lossy().to_string());
        }
    }

    // Inputs 1..N: each speech segment asset
    for seg in &plan.segments {
        args.push("-i".to_string());
        args.push(seg.resolved_file_path.to_string_lossy().to_string());
    }

    // Filtergraph
    args.push("-filter_complex".to_string());
    args.push(plan.filter_graph_desc.clone());

    args.push("-map".to_string());
    args.push("[out]".to_string());

    args.push("-c:a".to_string());
    args.push("pcm_s16le".to_string());

    args.push("-ar".to_string());
    args.push(plan.output_sample_rate.to_string());

    args.push("-ac".to_string());
    args.push(plan.output_channels.to_string());

    // Timeline duration limit
    let dur_secs = (plan.timeline_duration_us as f64) / 1_000_000.0;
    args.push("-t".to_string());
    args.push(format!("{dur_secs:.4}"));

    args.push(staged_path.to_string_lossy().to_string());

    let spec = ProcessSpec::new(discovered.canonical_path, args, DEFAULT_RENDER_TIMEOUT);

    // 5. Execute managed process
    let output = run_managed_process(spec, Some(&cancel_token))
        .await
        .map_err(|e| {
            let _ = std::fs::remove_file(&staged_path);
            let _ = std::fs::remove_dir_all(&staging_dir);
            DubbingMixError::FFmpegExecutionFailed(format!("Process launch error: {e}"))
        })?;

    if cancel_token.is_cancelled() {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(DubbingMixError::RenderCancelled);
    }

    if let Some(code) = output.exit_code {
        if code != 0 {
            let _ = std::fs::remove_file(&staged_path);
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(DubbingMixError::FFmpegExecutionFailed(format!(
                "FFmpeg exited with code {code}: {}",
                output.stderr.lines().last().unwrap_or("Unknown error")
            )));
        }
    } else {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(DubbingMixError::FFmpegExecutionFailed(
            "FFmpeg process was terminated abnormally".to_string(),
        ));
    }

    // 6. Analyze rendered audio quality & loudness
    let report = analyze_pcm_wav_file(&staged_path).inspect_err(|_| {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_dir_all(&staging_dir);
    })?;

    // 7. Atomically promote staged asset to durable storage
    let sha256 = promote_staged_mix_file(&staged_path, &durable_path).map_err(|e| {
        let _ = std::fs::remove_file(&staged_path);
        let _ = std::fs::remove_dir_all(&staging_dir);
        DubbingMixError::IoError(e)
    })?;

    let _ = std::fs::remove_dir_all(&staging_dir);
    let metadata = std::fs::metadata(&durable_path)?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let mut final_warnings = plan.warnings.clone();
    if report.clipping_samples > 0 {
        final_warnings.push(format!(
            "Detected {} saturated/clipping sample frames in mixed audio. Limiter recommended.",
            report.clipping_samples
        ));
    }

    Ok(DubbingMixResult {
        result_id: Uuid::new_v4().to_string(),
        project_id: plan.project_id.clone(),
        mix_asset_id: format!("mix_{cache_key}"),
        file_path: durable_path,
        sha256,
        actual_duration_us: report.duration_us,
        sample_rate: report.sample_rate,
        channels: report.channels,
        bit_depth: report.bit_depth,
        file_size_bytes: metadata.len(),
        peak_dbfs: report.peak_dbfs,
        rms_dbfs: report.rms_dbfs,
        integrated_lufs: report.integrated_lufs,
        clipping_samples: report.clipping_samples,
        warnings: final_warnings,
        created_at_epoch_ms: now_ms,
    })
}
